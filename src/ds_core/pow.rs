//! PoW 计算器 —— 基于 DeepSeek WASM 的 DeepSeekHashV1 算法实现
//!
//! 使用 wasmi（纯解释执行）替代 wasmtime，无需 JIT / cranelift，
//! 兼容低版本内核（如 4.4）的 LXC 容器环境。

use wasmi::core::ValType;
use wasmi::{Engine, ExternType, Linker, Module, Store};

// 复用 client 的 ChallengeData，避免重复定义
pub use crate::ds_core::client::ChallengeData as Challenge;

#[derive(Clone)]
pub struct PowSolver {
    module: Module,
    add_to_stack_name: String,
    alloc_name: String,
    solve_name: String,
}

#[derive(Debug)]
pub struct PowResult {
    pub algorithm: String,
    pub challenge: String,
    pub salt: String,
    pub answer: i64,
    pub signature: String,
    pub target_path: String,
}

impl PowResult {
    /// 将 PoW 结果转换为 base64 编码的 header
    pub fn to_header(&self) -> String {
        let json = serde_json::json!({
            "algorithm": self.algorithm,
            "challenge": self.challenge,
            "salt": self.salt,
            "answer": self.answer,
            "signature": self.signature,
            "target_path": self.target_path,
        });
        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            json.to_string().as_bytes(),
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PowError {
    #[error("WASM init failed: {0}")]
    WasmInit(String),
    #[error("WASM solve failed: no solution")]
    NoSolution,
    #[error("unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
    #[error("WASM execution error: {0}")]
    Execution(String),
}

impl PowSolver {
    pub fn new(wasm_bytes: &[u8]) -> Result<Self, PowError> {
        let engine = Engine::default();
        let module =
            Module::new(&engine, wasm_bytes).map_err(|e| PowError::WasmInit(e.to_string()))?;

        let exports: Vec<_> = module
            .exports()
            .map(|e| (e.name().to_string(), e.ty().clone()))
            .collect();

        let add_to_stack_name = find_export_by_names(
            &exports,
            &["__wbindgen_add_to_stack_pointer"],
            &[ValType::I32],
            &[ValType::I32],
        )
        .ok_or_else(|| {
            PowError::WasmInit("__wbindgen_add_to_stack_pointer not found".to_string())
        })?;

        // allocator: 优先找 __wbindgen_malloc，其次是签名匹配的 __wbindgen_export_*
        let alloc_name = find_export_by_names(
            &exports,
            &["__wbindgen_malloc"],
            &[ValType::I32, ValType::I32],
            &[ValType::I32],
        )
        .or_else(|| {
            find_export_by_prefix(
                &exports,
                "__wbindgen_export_",
                &[ValType::I32, ValType::I32],
                &[ValType::I32],
            )
        })
        .ok_or_else(|| PowError::WasmInit("allocator export not found".to_string()))?;

        // wasm_solve: 优先显式名称，再按唯一签名 (i32, i32, i32, i32, i32, f64) -> () 探测
        let solve_name = find_export_by_names(
            &exports,
            &["wasm_solve"],
            &[
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::I32,
                ValType::F64,
            ],
            &[],
        )
        .or_else(|| {
            let candidates: Vec<_> = exports
                .iter()
                .filter(|(_, ty)| {
                    matches_sig(
                        ty,
                        &[
                            ValType::I32,
                            ValType::I32,
                            ValType::I32,
                            ValType::I32,
                            ValType::I32,
                            ValType::F64,
                        ],
                        &[],
                    )
                })
                .map(|(name, _)| name.clone())
                .collect();
            if candidates.len() == 1 {
                Some(candidates.into_iter().next().unwrap())
            } else {
                None
            }
        })
        .ok_or_else(|| PowError::WasmInit("wasm_solve export not found".to_string()))?;

        Ok(Self {
            module,
            add_to_stack_name,
            alloc_name,
            solve_name,
        })
    }

    pub fn solve(&self, challenge: &Challenge) -> Result<PowResult, PowError> {
        if challenge.algorithm != "DeepSeekHashV1" {
            return Err(PowError::UnsupportedAlgorithm(challenge.algorithm.clone()));
        }

        let engine = self.module.engine();
        let mut store = Store::new(engine, ());
        let linker = Linker::new(engine);

        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|e| PowError::Execution(e.to_string()))?
            .start(&mut store)
            .map_err(|e| PowError::Execution(e.to_string()))?;

        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| PowError::Execution("memory not found".to_string()))?;
        let add_to_stack = instance
            .get_typed_func::<i32, i32>(&store, &self.add_to_stack_name)
            .map_err(|e| PowError::Execution(e.to_string()))?;
        let alloc = instance
            .get_typed_func::<(i32, i32), i32>(&store, &self.alloc_name)
            .map_err(|e| PowError::Execution(e.to_string()))?;
        let wasm_solve = instance
            .get_typed_func::<(i32, i32, i32, i32, i32, f64), ()>(&store, &self.solve_name)
            .map_err(|e| PowError::Execution(e.to_string()))?;

        let prefix = format!("{}_{}_", challenge.salt, challenge.expire_at);
        let retptr = add_to_stack
            .call(&mut store, -16)
            .map_err(|e| PowError::Execution(e.to_string()))?;

        let (ptr_challenge, len_challenge) =
            write_string(&mut store, &memory, &alloc, &challenge.challenge)?;
        let (ptr_prefix, len_prefix) = write_string(&mut store, &memory, &alloc, &prefix)?;

        wasm_solve
            .call(
                &mut store,
                (
                    retptr,
                    ptr_challenge,
                    len_challenge,
                    ptr_prefix,
                    len_prefix,
                    challenge.difficulty as f64,
                ),
            )
            .map_err(|e| PowError::Execution(e.to_string()))?;

        let mut status_buf = [0u8; 4];
        memory
            .read(&store, retptr as usize, &mut status_buf)
            .map_err(|e| PowError::Execution(e.to_string()))?;
        let status = i32::from_le_bytes(status_buf);

        let mut value_buf = [0u8; 8];
        memory
            .read(&store, (retptr + 8) as usize, &mut value_buf)
            .map_err(|e| PowError::Execution(e.to_string()))?;
        let value = f64::from_le_bytes(value_buf);

        add_to_stack
            .call(&mut store, 16)
            .map_err(|e| PowError::Execution(e.to_string()))?;

        if status == 0 {
            return Err(PowError::NoSolution);
        }

        Ok(PowResult {
            algorithm: challenge.algorithm.clone(),
            challenge: challenge.challenge.clone(),
            salt: challenge.salt.clone(),
            answer: value as i64,
            signature: challenge.signature.clone(),
            target_path: challenge.target_path.clone(),
        })
    }
}

fn write_string(
    store: &mut Store<()>,
    memory: &wasmi::Memory,
    alloc: &wasmi::TypedFunc<(i32, i32), i32>,
    text: &str,
) -> Result<(i32, i32), PowError> {
    let bytes = text.as_bytes();
    let len = bytes.len() as i32;
    let ptr = alloc
        .call(&mut *store, (len, 1))
        .map_err(|e| PowError::Execution(e.to_string()))?;
    memory
        .write(&mut *store, ptr as usize, bytes)
        .map_err(|e| PowError::Execution(e.to_string()))?;
    Ok((ptr, len))
}

fn matches_sig(ty: &ExternType, params: &[ValType], results: &[ValType]) -> bool {
    let Some(func_ty) = ty.func() else {
        return false;
    };
    let p = func_ty.params().to_vec();
    let r = func_ty.results().to_vec();
    p.len() == params.len()
        && r.len() == results.len()
        && p.iter()
            .zip(params.iter())
            .all(|(a, b)| std::mem::discriminant(a) == std::mem::discriminant(b))
        && r.iter()
            .zip(results.iter())
            .all(|(a, b)| std::mem::discriminant(a) == std::mem::discriminant(b))
}

fn find_export_by_names(
    exports: &[(String, ExternType)],
    names: &[&str],
    params: &[ValType],
    results: &[ValType],
) -> Option<String> {
    for name in names {
        if let Some((export_name, ty)) = exports.iter().find(|(n, _)| n == *name)
            && matches_sig(ty, params, results)
        {
            return Some(export_name.clone());
        }
    }
    None
}

fn find_export_by_prefix(
    exports: &[(String, ExternType)],
    prefix: &str,
    params: &[ValType],
    results: &[ValType],
) -> Option<String> {
    exports
        .iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .find(|(_, ty)| matches_sig(ty, params, results))
        .map(|(name, _)| name.clone())
}
