use cargo_tog::platform::{cache_key_host_fragment, host_arch, host_triple_hint, OsFamily};

pub fn run() {
    let os = OsFamily::current();
    println!("os={}", os.as_str());
    println!("arch={}", host_arch());
    println!("host_triple_hint={}", host_triple_hint());
    println!("cache_key_host={}", cache_key_host_fragment());
    println!();
    println!("# Suggested GitHub Actions key fragments");
    println!(
        "key: test-${{{{ runner.os }}}}-${{{{ runner.arch }}}}-{}",
        host_triple_hint()
    );
    println!("# Objects are NOT shared across OS/target — only within identical triples.");
}
