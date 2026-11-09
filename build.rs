// @Time    : 2026/11/5 15:40
// @Author  : fzf
// @FileName: build.rs
// @Software: Siskin

//! 资源依赖声明。
//!
//! `include_bytes!` 在编译期把图标烧进二进制。cargo 默认按文件 mtime 判断
//! 是否需要重编，而用文件管理器替换图片时 mtime 可能被保留成旧时间，
//! 结果就是「图换了但窗口图标没变」。这里显式声明资源目录，任何改动都会触发重编。

fn main() {
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=assets/siskin-256.png");
    println!("cargo:rerun-if-changed=assets/icons");
}
