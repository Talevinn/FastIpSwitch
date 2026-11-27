// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

fn main() {
    slint_build::compile("ui/app.slint").expect("échec de compilation de ui/app.slint");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set_manifest_file("app.manifest");
        res.set("ProductName", "FastIpSwitch");
        res.set("FileDescription", "FastIpSwitch");
        res.set("FileVersion", "0.1.0.0");
        res.set("ProductVersion", "0.1.0.0");
        res.set("CompanyName", "Valentin PICARD");
        res.set("LegalCopyright", "");
        res.set("OriginalFilename", "fastipswitch.exe");
        res.compile()
            .expect("échec de compilation des ressources Windows");
    }
}
