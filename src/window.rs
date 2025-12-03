use bevy::app::{App, Startup};
use bevy::camera::Camera2d;
use bevy::DefaultPlugins;
use bevy::prelude::Commands;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};

fn init() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup_camera_system)
        .add_systems(EguiPrimaryContextPass, ui_example_system)
        .run();
}

fn setup_camera_system(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn ui_example_system(mut contexts: EguiContexts) -> bevy::prelude::Result {
    let mut exposure = 0.0;
    let mut contrast = 0.0;
    let mut saturation = 0.0;
    egui::Window::new("Hello").show(contexts.ctx_mut()?, |ui| {
        ui.label("Exposure");
        ui.add(egui::Slider::new(&mut exposure, 0.0..=100.0));

        ui.label("Contrast");
        ui.add(egui::Slider::new(&mut contrast, 0.0..=100.0));

        ui.label("Saturation");
        ui.add(egui::Slider::new(&mut saturation, 0.0..=100.0));

        // ui.checkbox(&mut my_boolean, "Checkbox");
        // ui.add()


        ui.separator();

        // ui.image(my_image, [640.0, 480.0]);

        ui.collapsing("Click to see what is hidden!", |ui| {
            ui.label("Not much, as it turns out");
        });
    });
    Ok(())
}