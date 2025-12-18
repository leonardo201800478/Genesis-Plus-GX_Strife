use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // 1. Verifica se uma feature SIMD foi explicitamente forçada via linha de comando (ex: --features avx2)
    let forced_avx512 = env::var_os("CARGO_FEATURE_AVX512").is_some();
    let forced_avx2 = env::var_os("CARGO_FEATURE_AVX2").is_some();
    let forced_sse41 = env::var_os("CARGO_FEATURE_SSE41").is_some();

    if forced_avx512 || forced_avx2 || forced_sse41 {
        // Usa a configuração que o usuário explicitamente pediu
        if forced_avx512 {
            println!("cargo:rustc-cfg=feature_avx512");
            println!("cargo:rustc-cpu=native");
        } else if forced_avx2 {
            println!("cargo:rustc-cfg=feature_avx2");
            println!("cargo:rustc-cpu=haswell");
        } else if forced_sse41 {
            println!("cargo:rustc-cfg=feature_sse41");
        }
    } else if cfg!(target_arch = "x86_64") {
        // 2. Caso contrário, faz detecção automática da CPU HOST (apenas para x86_64)
        // AVX-512 (CPUs Intel/AMD muito recentes)
        if std::arch::is_x86_feature_detected!("avx512f") {
            println!("cargo:rustc-cfg=feature_avx512");
            println!("cargo:rustc-cpu=native");
        }
        // AVX2 (Intel Haswell+, AMD Ryzen+ - Muito comum em CPUs modernas)
        else if std::arch::is_x86_feature_detected!("avx2") {
            println!("cargo:rustc-cfg=feature_avx2");
            println!("cargo:rustc-cpu=haswell");
        }
        // SSE4.1 (Fallback para CPUs mais antigas)
        else if std::arch::is_x86_feature_detected!("sse4.1") {
            println!("cargo:rustc-cfg=feature_sse41");
        }
    }

    // Otimização de linking para Linux (remove símbolos de debug do binário final)
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-Wl,--strip-all"); // Remove símbolos de debug
    }
}
