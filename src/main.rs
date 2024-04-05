#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
                                                                   //IO
use eframe::egui::debug_text::print;
use rev_lines::RevLines;
use std::fs::File;
use std::hash::Hash;
use std::io::prelude::*;
use std::iter::Map;
use std::path::Path;

//UI
use eframe::egui;
use tokio::{runtime::Runtime, time};
//Timing
use std::time::{Duration, Instant, SystemTime};
//Math
use std::cmp;
//Data Structures
use std::collections::HashMap;
//Buttplug Lib
use buttplug::{
    client::{device::ScalarValueCommand, ButtplugClient, ButtplugClientError},
    core::{
        connector::new_json_ws_client_connector, message::ClientGenericDeviceMessageAttributes,
    },
};
// use buttplug::core::connector::ButtplugConnectorError;
#[derive(Debug)]
enum BPActionType {
    Stop,
    Vibrate { strength: f64, motor: i8 },
    Power { strength: f64, motor: i8 },
    Stroke,
}
#[derive(Debug)]
enum BPEffectorType {
    Vibrates { intensity: f64 },
    Strokes { amplitude: f64 },
}
#[derive(Debug)]
struct BPSimEvent {
    pub finished: bool,
    time_remaining: Duration,
    action: BPActionType,
}

impl BPSimEvent {
    pub fn new(initial_duration: Duration, action: BPActionType) -> BPSimEvent {
        BPSimEvent {
            finished: false,
            time_remaining: initial_duration,
            action: action,
        }
    }
    pub fn new_stop_event() -> BPSimEvent {
        BPSimEvent {
            finished: true,
            time_remaining: Duration::ZERO,
            action: BPActionType::Stop,
        }
    }
    pub fn pass_time(&mut self, time_passed: Duration) {
        if (self.finished) {
            return;
        }
        self.time_remaining = match self.time_remaining.checked_sub(time_passed) {
            None => {
                self.finished = true;
                Duration::ZERO
            }
            Some(time_left) => time_left,
        }
    }
}
#[derive(Debug)]
struct BPEffector {
    effector_type: BPEffectorType,
    index: i8,
}

impl BPEffector
{
  pub fn new(effector_type:BPEffectorType, index:i8) -> BPEffector
  {
    BPEffector
    {
      effector_type,
      index
    }
  }
}

#[derive(Debug)]
struct BPSimulator {
    events: Vec<BPSimEvent>,
    effectors: Vec<BPEffector>,
    last_sim_instant: Instant,
    formula_threshold: f64,
    formula_half_life_vib: Duration,
    formula_linear_reduction_vib: f64,
    formula_floor_cache: HashMap<i8, f64>,
}

impl Default for BPSimulator {
    fn default() -> BPSimulator {
        BPSimulator {
            events: Vec::new(),
            effectors: Vec::new(),
            last_sim_instant: std::time::Instant::now(),
            formula_threshold: 0.01 as f64,
            formula_half_life_vib: Duration::from_millis(200),
            formula_linear_reduction_vib: 0.005 as f64,
            formula_floor_cache: HashMap::new(),
        }
    }
}

impl BPSimulator {
    pub fn new() -> BPSimulator {
        Default::default()
    }
    pub fn add_event(&mut self, event: BPSimEvent) {
        println!("Event added: {event:?}");
        //process the effects of adding this event
        //add initial value to effector
        match event.action {
            BPActionType::Vibrate { strength, motor } => {
                println!("Adding vibration event");
                self.update_intensity_floor(motor, strength);
            }
            BPActionType::Power { strength, motor } => {
                println!("Adding vibration power event");
                //TODO: End all other power events
                self.update_intensity_floor(motor, strength);
            }
            BPActionType::Stop => {
                println!("Stop recieved, clearing all events and resetting all intensities");
                self.force_stop();
                return;
            }
            BPActionType::Stroke => {
                println!("Stroke event not yet supported");
            }
        };
        self.events.push(event);
    }
    pub fn add_effector(&mut self, effector: BPEffector) {
        println!("Effector added: {effector:?}");
        match effector.effector_type {
            BPEffectorType::Vibrates { .. } => {
                self.formula_floor_cache.insert(effector.index, 0 as f64)
            }
            BPEffectorType::Strokes { .. } => None,
        };
        self.effectors.push(effector);
    }
    pub fn process_tick(&mut self, current_instant: Instant) {
        let time_passed = match current_instant.checked_duration_since(self.last_sim_instant) {
            None => {
                println!("Error: went back in time");
                return;
            }
            Some(dur) => dur,
        };
        if time_passed > Duration::from_millis(1000 / 10) {
            println!("Unusually long tick: {time_passed:?}");
        }
        self.last_sim_instant = current_instant;
        //Run through and update effector states
        self.update_effectors(time_passed);
        //Update time remaining on events
        self.progress_event_times(time_passed);
        //Cull dead events
        self.cull_old_events();
    }
    fn progress_event_times(&mut self, time_passed: Duration) {
        for event in self.events.iter_mut() {
            event.pass_time(time_passed);
        }
    }
    fn update_effectors(&mut self, time_passed: Duration) {
        for effector in self.effectors.iter_mut() {
            match effector.effector_type {
                BPEffectorType::Vibrates { ref mut intensity } => {
                    //Half life decay
                    *intensity = BPSimulator::calc_intensity_decay(
                        time_passed,
                        *intensity,
                        self.formula_linear_reduction_vib,
                        self.formula_half_life_vib,
                    );
                    debug_assert!(
                        match self.formula_floor_cache.get(&effector.index) {
                            None => false,
                            Some(_) => true,
                        },
                        "This vibrator was incorrectly initialized! It doesn't have a floor value."
                    );
                    //Must be at minimum equal to currently active events
                    *intensity = f64::max(
                        *intensity,
                        *(self.formula_floor_cache.get(&effector.index).unwrap()),
                    );
                }
                BPEffectorType::Strokes { amplitude } => {
                    //Not implemented yet
                }
            }
        }
    }
    fn cull_old_events(&mut self) {
        let mut index = 0;
        //perform last actions of finished events
        while index < self.events.len() {
            match self.events.get(index) {
                None => {
                    println!("Error: index out of bounds during culling.")
                }
                Some(ev) => {
                    if !ev.finished {
                        index +=1;
                        continue;
                    }
                }
            }
            match self.events.get(index).unwrap().action {
                BPActionType::Vibrate { strength, motor } => {
                    println!("Removing vibration event");
                    //TODO
                    self.update_intensity_floor(motor, -strength);
                }
                BPActionType::Power { strength, motor } => {
                    println!("Removing vibration power event");
                    //TODO
                    self.update_intensity_floor(motor, -strength);
                }
                BPActionType::Stop => {
                    println!("Stop recieved, clearing all events and resetting all intensities");
                    self.force_stop();
                    return;
                }
                BPActionType::Stroke => {
                    println!("Stroke event not yet supported");
                }
            }
          index +=1;
        }
        //remove finished events
        let precull_event_count = self.events.len();
        self.events.retain(|ev: &BPSimEvent| !ev.finished);
        if (precull_event_count > self.events.len()) {
            println!("{} events culled", precull_event_count - self.events.len());
        }
    }
    //Models the decrease in vibrator intensity based on the half life formula, with a minor linear offset
    // I_n = I_c * (1/2)^(deltaT/half_life) - offset
    pub fn calc_intensity_decay(
        time_passed: Duration,
        current_intensity: f64,
        linear_reduction: f64,
        half_life: Duration,
    ) -> f64 {
        // println!("Calculating Decay with params:\ntime_passed: {}, current_intensity: {}, linear_reduction: {}, half_life: {}", time_passed.as_secs_f64(), current_intensity, linear_reduction, half_life.as_secs_f64());
        let mut new_intensity: f64 = current_intensity;
        let half_life_decay = f64::powf(
            0.5 as f64,
            (time_passed.as_secs_f64() / half_life.as_secs_f64()),
        );
        // println!("Half life decay factor: {}", half_life_decay);
        new_intensity = f64::max(new_intensity * half_life_decay - linear_reduction, 0 as f64);
        // println!(
        //     "Final intensity: {} from initial intensity of {}",
        //     new_intensity, current_intensity
        // );
        return new_intensity;
    }

    fn update_intensity_floor(&mut self, index: i8, intensity_change: f64) {
      println!("Updating intensity for motor {} by {}", index, intensity_change);
        if index == -1 {
            for (_, original_intensity) in self.formula_floor_cache.iter_mut() {
                *original_intensity = f64::max(*original_intensity + intensity_change, 0 as f64);
            }
        } else {
            match self.formula_floor_cache.get_mut(&index) {
                None => {
                    println!("Missing intensity floor for index {}", index)
                }
                Some(intensity) => {
                    *intensity = f64::max(*intensity + intensity_change, 0 as f64);
                }
            }
        }
    }

    pub fn get_vibrator_intensities(&self) -> Vec<f64>
    {
      let mut intensities: Vec<f64> = Vec::new();
      for effector in self.effectors.as_slice()
      {
        match effector.effector_type
        {
          BPEffectorType::Vibrates { intensity } => 
          {
            if(effector.index as usize > intensities.len()+1){
              println!("Effectors out of order");
              continue;
            }
            intensities.insert(effector.index as usize, intensity);
          },
          BPEffectorType::Strokes { amplitude } => {
            println!("Strokers not yet implemented");
          }
        }
      }
    //   println!("Intensities: {intensities:?}");
      return intensities;
    }
    //Clear all events, set all intensities to 0
    pub fn force_stop(&mut self) {
        println!("Force stopping");
        println!("Events to remove: {}", self.events.len());
        self.events.clear();
        for (_, intensity) in self.formula_floor_cache.iter_mut() {
            *intensity = 0 as f64;
        }
        //TODO: Force stop for other components
    }

    pub fn reset_for_new_device(&mut self)
    {
      self.force_stop();
      self.effectors.clear();
      self.formula_floor_cache.clear();
    }

    pub fn add_multiple_vib_effectors(&mut self, num_motors: usize)
    {
        println!("Adding {} vibrational effectors", num_motors);
        let mut vib_index: usize = 0;
        while vib_index < num_motors
        {
            self.add_effector(BPEffector::new(BPEffectorType::Vibrates { intensity: 0 as f64 }, vib_index as i8));
            vib_index += 1;
        }
        //TODO: Other effectors
        println!("Done adding effectors! Total of {} added", vib_index);

    }
}

pub struct BPIntifaceClient {
    client: Option<ButtplugClient>,
    rt: Option<Runtime>,
}

impl BPIntifaceClient {
    pub fn test() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        // Call the asynchronous connect method using the runtime.
        let _inner = rt.block_on(test_buttplug());
    }
    pub fn connect(&mut self) {
        self.rt = Some(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap(),
        );

        // Call the asynchronous connect method using the runtime.
        self.client = Some(
            self.rt
                .as_mut()
                .unwrap()
                .block_on(connect_buttplug())
                .unwrap(),
        );
    }
    pub fn vibrate(&mut self) {
        self.rt
            .as_mut()
            .unwrap()
            .block_on(vibrate_buttplug(&self.client.as_mut().unwrap()));
    }
    pub fn stop(&mut self) {
        self.rt
            .as_mut()
            .unwrap()
            .block_on(stop_buttplug(&self.client.as_mut().unwrap()));
    }

    pub fn set_device_vibration_strengths(&mut self, strengths:Vec<f64>)
    {
      self.rt
            .as_mut()
            .unwrap()
            .block_on(device_set_vibration_strengths(&self.client.as_mut().unwrap(), strengths));
    }
    pub fn num_vibrator_motors(&mut self) -> usize
    {
        match &self.client
        {
            None => {
                println!("Client not connected!");
                0 as usize
            }
            Some(bp_client) => {
                bp_client.devices()[0].vibrate_attributes().len()
            }
        }
    }
}

struct MyApp {
    name: String,
    age: u32,
    bp_client: Option<BPIntifaceClient>,
    bp_sim: BPSimulator,
    update_ticks: u32,
    device_order_period: Duration,
    device_last_order_instant: Instant,
    file_text: Option<String>,
    debug_event_millis: u64,
    debug_event_strength: f64,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "Arthur".to_owned(),
            age: 42,
            bp_client: None,
            bp_sim: Default::default(),
            update_ticks: 0,
            device_order_period: Duration::from_millis(100),
            device_last_order_instant: Instant::now(),
            file_text: None,
            debug_event_millis: 500,
            debug_event_strength: 0.5,
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
        self.update_ticks += 1;
        match self.bp_client.as_mut()
        {
          None => {},
          Some(client) => {
            self.bp_sim.process_tick(std::time::Instant::now());
            if(Instant::now() - self.device_last_order_instant >= self.device_order_period)
            {
              self.device_last_order_instant = Instant::now();
              client.set_device_vibration_strengths(self.bp_sim.get_vibrator_intensities());
            }
          },
        };
        ctx.request_repaint_after(std::time::Duration::from_micros(
            (1.0 / 60.0 * 1000000.0) as u64,
        ));
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
                self.bp_client = Some(BPIntifaceClient {
                    client: None,
                    rt: None,
                });
                self.bp_client.as_mut().unwrap().connect();
                self.bp_sim.reset_for_new_device();
                //TODO: Make this line work
                self.bp_sim.add_multiple_vib_effectors(self.bp_client.as_mut().unwrap().num_vibrator_motors());
            }
            if ui.button("Vibrate").clicked() {
                match self.bp_client.as_mut() {
                    None => println!("Not connected"),
                    Some(client) => client.vibrate(),
                }
                // self.bp_client.as_mut().unwrap().vibrate();
            }
            if ui.button("Stop").clicked() {
                self.bp_client.as_mut().unwrap().stop();
            }
            if ui.button("Display File").clicked() {
                let path = Path::new("filetest.txt");
                let display = path.display();

                let mut file = match File::open(&path) {
                    Err(why) => panic!("couldn't open {}: {}", display, why),
                    Ok(file) => file,
                };

                self.file_text = Some(String::new());
                match file.read_to_string(&mut self.file_text.as_mut().unwrap()) {
                    Err(why) => panic!("couldn't read {}: {}", display, why),
                    Ok(_) => print!(
                        "{} contains:\n{}",
                        display,
                        self.file_text.as_mut().unwrap()
                    ),
                };
                println!("\n");
            }
            //Debug panel
            ui.add(egui::Slider::new(&mut self.debug_event_millis, 100..=5000).text("Debug Event Duration (millis)"));
            ui.add(egui::Slider::new(&mut self.debug_event_strength, 0.001..=1.0).text("Debug Event Strength"));

            if ui.button("Add Debug Event").clicked() {
              self.bp_sim.add_event(
                BPSimEvent::new(Duration::from_millis(self.debug_event_millis), BPActionType::Vibrate { strength: self.debug_event_strength, motor: -1 as i8 })
              )
            }
            if ui.button("Add Debug Stop").clicked() {
              self.bp_sim.add_event(BPSimEvent::new_stop_event());
            }
            // ui.label(format!("Hello '{}', age {}", self.name, self.age));
            ui.label(format!("Ticks passed: {}", self.update_ticks));
            match &self.file_text {
                None => ui.label(format!("No file currently loaded.")),
                Some(text_string) => ui.label(format!("File contains:\n{}", text_string)),
            };

            ui.image(egui::include_image!("../resources/neco.png"));
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([315.0, 480.0]),
        ..Default::default()
    };

    // let path = Path::new("filetest.txt");
    // let display = path.display();

    // let mut file = match File::open(&path) {
    //     Err(why) => panic!("couldn't open {}: {}", display, why),
    //     Ok(file) => file,
    // };

    // let mut s = String::new();
    // match file.read_to_string(&mut s) {
    //     Err(why) => panic!("couldn't read {}: {}", display, why),
    //     Ok(_) => print!("{} contains:\n{}", display, s),
    // }
    // println!("\n");
    // let rev_lines = RevLines::new(file);
    // for line in rev_lines {
    //     match line {
    //         Err(ref why) => println!("Error when reading {}: {}", display, why),
    //         Ok(ref line_str) => println!("{}", line_str),
    //     }
    //     println!("{:?}", line);
    // }

    // let path2 = Path::new("resources/filetest2.txt");
    // let display2 = path2.display();

    // let mut file2 = match File::open(&path2) {
    //     Err(why) => panic!("couldn't open {}: {}", display2, why),
    //     Ok(file2) => file2,
    // };

    // let mut s2 = String::new();
    // match file2.read_to_string(&mut s2) {
    //     Err(why) => panic!("couldn't read {}: {}", display2, why),
    //     Ok(_) => print!("{} contains:\n{}", display2, s),
    // }
    // println!("\n");

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

// fn load_device_effectors_to_sim(mut bp_sim: &BPSimulator, client: &ButtplugClient)
// {
//   println!("Loading devices to sim");
//   // let bp_client= match bpi_client.client.as_mut()
//   // {
//   //   None => {
//   //     println!("There is no client bound!");
//   //     return;
//   //   },
//   //   Some(bpc) => bpc,
//   // };
//   let client_device = client.devices()[0];
//   //Load vibrators
//   let mut vib_index: i8 = 0;
//   let vibrator_count = client_device.vibrate_attributes().len() as i8;
//   while vib_index < vibrator_count
//   {
//     bp_sim.add_effector(BPEffector::new(BPEffectorType::Vibrates { intensity: 0 as f64 }, vib_index));
//     vib_index += 1;
//   }
//   //TODO: Other effectors
//   println!("Done adding effectors! Total of {} added", vib_index);
// }

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
    let vibrator_count = test_client_device.vibrate_attributes().len();

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

async fn connect_buttplug() -> Result<ButtplugClient, ButtplugClientError> {
    println!("Attempting Connection");
    let connector = new_json_ws_client_connector("ws://localhost:12345");

    let client = ButtplugClient::new("Beyond All Buttplug Client");
    client.connect(connector).await?;

    println!("Connected to Intiface");

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

async fn vibrate_buttplug(client: &ButtplugClient) -> Result<bool, ButtplugClientError> {
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
    let vibrator_count = test_client_device.vibrate_attributes().len();

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

async fn stop_buttplug(client: &ButtplugClient) -> Result<bool, ButtplugClientError> {
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

async fn device_set_vibration_strengths(client: &ButtplugClient, mut strengths: Vec<f64>) -> Result<(), ButtplugClientError>
{
//   println!("Setting vibrators to: {strengths:?}");
  let client_device = &client.devices()[0];
  let vibrator_count = client_device.vibrate_attributes().len();
//   println!(
//       "{} has {} vibrators.",
//       client_device.name(),
//       vibrator_count,
//   );
  if strengths.len()!=vibrator_count
  {
    println!("Note: Number of vibrator settings different from device.\n
    {} strengths sent, {} motors on device", strengths.len(), vibrator_count);
    if strengths.len() > vibrator_count {
      strengths.truncate(vibrator_count);
    }
    else {
        while strengths.len() < vibrator_count{
          strengths.push(0 as f64);
        }
    }
  }
  
  //Send the command
  client_device
            .vibrate(&ScalarValueCommand::ScalarValueVec(strengths))
            .await?;

  Ok(())
}

async fn device_stop(client: &ButtplugClient) -> Result<(), ButtplugClientError>
{
  println!("Stopping all movement");
  let client_device = &client.devices()[0];
  client_device.stop().await?;
  println!("Stopped");
  Ok(())
}