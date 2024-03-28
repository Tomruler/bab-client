#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::fs::File;
use std::path::Path;
use std::io::prelude::*;
use rev_lines::RevLines;

use eframe::egui;
use tokio::runtime::Runtime;

use std::time::{Duration, SystemTime};

use buttplug::{
    client::{device::ScalarValueCommand, ButtplugClient, ButtplugClientError},
    core::{
      connector::{
        new_json_ws_client_connector,
      },
      message::{ClientGenericDeviceMessageAttributes},
    },
  };
// use buttplug::core::connector::ButtplugConnectorError;

pub struct BPIntifaceClient
{
  client: Option<ButtplugClient>,
  rt: Option<Runtime>,
}

impl BPIntifaceClient
{
  pub fn test(){
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // Call the asynchronous connect method using the runtime.
    let _inner = rt.block_on(test_buttplug());
  } 
  pub fn connect(&mut self){
    self.rt = Some(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap());

    // Call the asynchronous connect method using the runtime.
    self.client = Some(self.rt.as_mut().unwrap().block_on(connect_buttplug()).unwrap());
  }
  pub fn vibrate(&mut self)
  {
    self.rt.as_mut().unwrap().block_on(vibrate_buttplug(&self.client.as_mut().unwrap()));
  }
  pub fn stop(&mut self)
  {
    self.rt.as_mut().unwrap().block_on(stop_buttplug(&self.client.as_mut().unwrap()));
  }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    let path = Path::new("filetest.txt");
    let display = path.display();

    let mut file = match File::open(&path){
        Err(why) => panic!("couldn't open {}: {}", display, why),
        Ok(file) => file,
    };

    let mut s = String::new();
    match file.read_to_string(&mut s) {
        Err(why) => panic!("couldn't read {}: {}", display, why),
        Ok(_) => print!("{} contains:\n{}", display, s),
    }
    println!("\n");
    let rev_lines = RevLines::new(file);
    for line in rev_lines {
      match line{
        Err(ref why) => println!("Error when reading {}: {}", display, why),
        Ok(ref line_str) => println!("{}", line_str),
      }
      println!("{:?}", line);
      
    }

    let path2 = Path::new("resources/filetest2.txt");
    let display2 = path2.display();

    let mut file2 = match File::open(&path2){
        Err(why) => panic!("couldn't open {}: {}", display2, why),
        Ok(file2) => file2,
    };

    let mut s2 = String::new();
    match file2.read_to_string(&mut s2) {
        Err(why) => panic!("couldn't read {}: {}", display2, why),
        Ok(_) => print!("{} contains:\n{}", display2, s),
    }
    println!("\n");

    // First try at executing async code in sync context
    // let rt = tokio::runtime::Builder::new_current_thread().enable_all().build();
    // let inner = rt.block_on(test_buttplug());
    BPIntifaceClient::test();
    // let bp_client = BPIntifaceClient;
    // bp_client.connect();
    eframe::run_native(
        "Beyond All Buttplug Client",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Box::<MyApp>::default()
            // Box::<MyApp>::bound(bp_client)
        }),
    )
}

struct MyApp {
    name: String,
    age: u32,
    bp_client: Option<BPIntifaceClient>,
    update_ticks: u32,
    file_text: Option<String>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "Arthur".to_owned(),
            age: 42,
            bp_client: None,
            update_ticks: 0,
            file_text: None,
        }
    }

    // fn bound(client: BPIntifaceClient) -> Self {
    //   Self {
    //       name: "Arthur".to_owned(),
    //       age: 42,
    //       bp_client: client,
    //   }
    // }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_ticks+=1;
        ctx.request_repaint_after(std::time::Duration::from_micros((1.0/60.0*1000000.0) as u64));
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Beyond All Buttplug Client");
            // ui.horizontal(|ui| {
            //     let name_label = ui.label("Your name: ");
            //     ui.text_edit_singleline(&mut self.name)
            //         .labelled_by(name_label.id);
            // });
            // ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            // if ui.button("Increment").clicked() {
            //     self.age += 1;
            //     // self.bp_client.as_mut().unwrap().vibrate();
            // }

            if ui.button("Connect").clicked() {
              self.bp_client = Some(BPIntifaceClient{
                client:None, rt:None
              });
              self.bp_client.as_mut().unwrap().connect();
            }
            if ui.button("Vibrate").clicked() {
              match self.bp_client.as_mut()
              {
                None => println!("Not connected"),
                Some(client) => client.vibrate(),
              }
              // self.bp_client.as_mut().unwrap().vibrate();
          }
            if ui.button("Stop").clicked() {
              self.bp_client.as_mut().unwrap().stop();
            }
            if ui.button("Display File").clicked()
            {
              let path = Path::new("filetest.txt");
              let display = path.display();

              let mut file = match File::open(&path){
                  Err(why) => panic!("couldn't open {}: {}", display, why),
                  Ok(file) => file,
              };

              self.file_text = Some(String::new());
              match file.read_to_string(&mut self.file_text.as_mut().unwrap()) {
                  Err(why) => panic!("couldn't read {}: {}", display, why),
                  Ok(_) => print!("{} contains:\n{}", display, self.file_text.as_mut().unwrap()),
              };
              println!("\n");
            }

            // ui.label(format!("Hello '{}', age {}", self.name, self.age));
            ui.label(format!("Ticks passed: {}", self.update_ticks));
            match &self.file_text{
              None => ui.label(format!("No file currently loaded.")),
              Some(text_string) => ui.label(format!("File contains:\n{}", text_string)),
            };
            
            ui.image(egui::include_image!(
                "../resources/neco.png"
            ));
        });
    }
}

async fn test_buttplug() -> anyhow::Result<()> {
    println!("Entered main");
    let connector = new_json_ws_client_connector("ws://localhost:12345");
  
    let client = ButtplugClient::new("Example Client");
    client.connect(connector).await?;
  
    println!("Connected!");
  
    // You usually shouldn't run Start/Stop scanning back-to-back like
    // this, but with TestDevice we know our device will be found when we
    // call StartScanning, so we can get away with it.
    client.start_scanning().await?;
    client.stop_scanning().await?;
    println!("Client currently knows about these devices:");
    for device in client.devices() {
      println!("- {}", device.name());
    }
    // wait_for_input().await;
  
    for device in client.devices() {
      fn print_attrs(attrs: &Vec<ClientGenericDeviceMessageAttributes>) {
        for attr in attrs {
          println!(
            "{}: {} - Steps: {}",
            attr.actuator_type(),
            attr.feature_descriptor(),
            attr.step_count()
          );
        }
      }
      println!("{} supports these actions:", device.name());
      if let Some(attrs) = device.message_attributes().scalar_cmd() {
        print_attrs(attrs);
      }
      print_attrs(&device.rotate_attributes());
      print_attrs(&device.linear_attributes());
      println!("Battery: {}", device.has_battery_level());
      println!("RSSI: {}", device.has_rssi_level());
    }
  
    println!("Sending commands");
  
    // Now that we know the message types for our connected device, we
    // can send a message over! Seeing as we want to stick with the
    // modern generic messages, we'll go with VibrateCmd.
    //
    // There's a couple of ways to send this message.
    let test_client_device = &client.devices()[0];
  
    // We can use the convenience functions on ButtplugClientDevice to
    // send the message. This version sets all of the motors on a
    // vibrating device to the same speed.
    test_client_device
      .vibrate(&ScalarValueCommand::ScalarValue(1.0))
      .await?;
  
    // If we wanted to just set one motor on and the other off, we could
    // try this version that uses an array. It'll throw an exception if
    // the array isn't the same size as the number of motors available as
    // denoted by FeatureCount, though.
    //
    // You can get the vibrator count using the following code, though we
    // know it's 2 so we don't really have to use it.
    let vibrator_count = test_client_device
      .vibrate_attributes()
      .len();
  
    println!(
      "{} has {} vibrators.",
      test_client_device.name(),
      vibrator_count,
    );
  
    // Just set all of the vibrators to full speed.
    if vibrator_count > 1 {
      test_client_device
        .vibrate(&ScalarValueCommand::ScalarValueVec(vec![1.0, 0.0]))
        .await?;
    } else {
      println!("Device does not have > 1 vibrators, not running multiple vibrator test.");
    }
  
    // wait_for_input().await;
    println!("Disconnecting");
    // And now we disconnect as usual.
    client.disconnect().await?;
    println!("Trying error");
    // If we try to send a command to a device after the client has
    // disconnected, we'll get an exception thrown.
    let vibrate_result = test_client_device
      .vibrate(&ScalarValueCommand::ScalarValue(1.0))
      .await;
    if let Err(ButtplugClientError::ButtplugConnectorError(error)) = vibrate_result {
      println!("Tried to send after disconnection! Error: ");
      println!("{}", error);
    }
    Ok(())
}

async fn connect_buttplug() -> Result<ButtplugClient, ButtplugClientError>
{
    println!("Attempting Connection");
    let connector = new_json_ws_client_connector("ws://localhost:12345");
  
    let client = ButtplugClient::new("Example Client");
    client.connect(connector).await?;
  
    println!("Connected!");
  
    // You usually shouldn't run Start/Stop scanning back-to-back like
    // this, but with TestDevice we know our device will be found when we
    // call StartScanning, so we can get away with it.
    client.start_scanning().await?;
    client.stop_scanning().await?;
    println!("Client currently knows about these devices:");
    for device in client.devices() {
      println!("- {}", device.name());
    }
    // wait_for_input().await;
  
    for device in client.devices() {
      fn print_attrs(attrs: &Vec<ClientGenericDeviceMessageAttributes>) {
        for attr in attrs {
          println!(
            "{}: {} - Steps: {}",
            attr.actuator_type(),
            attr.feature_descriptor(),
            attr.step_count()
          );
        }
      }
      println!("{} supports these actions:", device.name());
      if let Some(attrs) = device.message_attributes().scalar_cmd() {
        print_attrs(attrs);
      }
      print_attrs(&device.rotate_attributes());
      print_attrs(&device.linear_attributes());
      println!("Battery: {}", device.has_battery_level());
      println!("RSSI: {}", device.has_rssi_level());
    }
    Ok(client)
}

async fn vibrate_buttplug(client: &ButtplugClient) -> Result<bool, ButtplugClientError>
{

  println!("Sending commands");
  
    // Now that we know the message types for our connected device, we
    // can send a message over! Seeing as we want to stick with the
    // modern generic messages, we'll go with VibrateCmd.
    //
    // There's a couple of ways to send this message.
    let test_client_device = &client.devices()[0];
  
    // We can use the convenience functions on ButtplugClientDevice to
    // send the message. This version sets all of the motors on a
    // vibrating device to the same speed.
    test_client_device
      .vibrate(&ScalarValueCommand::ScalarValue(1.0))
      .await?;
  
    // If we wanted to just set one motor on and the other off, we could
    // try this version that uses an array. It'll throw an exception if
    // the array isn't the same size as the number of motors available as
    // denoted by FeatureCount, though.
    //
    // You can get the vibrator count using the following code, though we
    // know it's 2 so we don't really have to use it.
    let vibrator_count = test_client_device
      .vibrate_attributes()
      .len();
  
    println!(
      "{} has {} vibrators.",
      test_client_device.name(),
      vibrator_count,
    );
  
    // Just set all of the vibrators to full speed.
    if vibrator_count > 1 {
      test_client_device
        .vibrate(&ScalarValueCommand::ScalarValueVec(vec![1.0, 0.0]))
        .await?;
    } else {
      println!("Device does not have > 1 vibrators, not running multiple vibrator test.");
    }
    Ok(true)
    // wait_for_input().await;
    // println!("Disconnecting");
    // // And now we disconnect as usual.
    // println!("Trying error");
    // // If we try to send a command to a device after the client has
    // // disconnected, we'll get an exception thrown.
    // let vibrate_result = test_client_device
    //   .vibrate(&ScalarValueCommand::ScalarValue(1.0))
    //   .await;
    // if let Err(ButtplugClientError::ButtplugConnectorError(error)) = vibrate_result {
    //   println!("Tried to send after disconnection! Error: ");
    //   println!("{}", error);
    // }
}

async fn stop_buttplug(client: &ButtplugClient) -> Result<bool, ButtplugClientError>
{
  println!("Sending commands");
  
    // Now that we know the message types for our connected device, we
    // can send a message over! Seeing as we want to stick with the
    // modern generic messages, we'll go with VibrateCmd.
    //
    // There's a couple of ways to send this message.
    let test_client_device = &client.devices()[0];
    test_client_device.stop().await?;
  println!("Stopped");
  Ok(true)
}