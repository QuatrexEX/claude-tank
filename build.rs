fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Claude Tank");
        res.set("FileDescription", "Claude plan usage monitor");
        res.set("CompanyName", "Quatrex");
        res.compile().expect("Failed to compile Windows resources");
    }
}
