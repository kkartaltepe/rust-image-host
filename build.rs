use std::env;

fn main() {
	let target = env::var("TARGET").unwrap();
	if target.contains("windows") {
    	println!("cargo:rustc-flags=-l user32 -l gdi32");
	}
}
