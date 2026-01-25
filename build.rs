fn main() {
    let config = slint_build::CompilerConfiguration::new().with_library_paths(
        std::collections::HashMap::from([(
            "material".to_string(),
            std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
                .join("material-1.0/material.slint"),
        )]),
    );

    // Compile taskbar panel
    slint_build::compile_with_config("ui/panels/taskbar/taskbar.slint", config.clone())
        .expect("Taskbar build failed");

    // Compile music player selector panel
    slint_build::compile_with_config(
        "ui/panels/media_selector/media_selector.slint",
        config.clone(),
    )
    .expect("Music player selector build failed");

    // Add more panels as you create them:
    // slint_build::compile_with_config("ui/panels/menu/menu.slint", config.clone()).expect("Menu build failed");
    // slint_build::compile_with_config("ui/panels/osd/osd.slint", config.clone()).expect("OSD build failed");
}
