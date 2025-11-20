fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("cosmic-dark".into());
    
    // Compile taskbar panel
    slint_build::compile_with_config("ui/panels/taskbar/taskbar.slint", config.clone()).expect("Taskbar build failed");
    
    // Add more panels as you create them:
    // slint_build::compile_with_config("ui/panels/menu/menu.slint", config.clone()).expect("Menu build failed");
    // slint_build::compile_with_config("ui/panels/osd/osd.slint", config.clone()).expect("OSD build failed");
}