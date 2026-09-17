//! `alluno-input probe`, `alluno-input key <name>`, `alluno-input mouse`,
//! `alluno-input pad <profile> [seconds]`, `alluno-input pen`, `alluno-input touch`.

use std::time::{Duration, Instant};

use alluno_input::{
    GamepadOutput, GamepadProfile, GamepadState, Host, Key, Options, PenState, Runtime,
    TouchContact, TouchState, buttons,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("probe") => probe(),
        Some("key") => key(args.get(1).map(String::as_str)),
        Some("pad") => pad(
            args.get(1).map(String::as_str),
            args.get(2).map(String::as_str),
        ),
        Some("mouse") => mouse(),
        Some("pen") => pen(),
        Some("touch") => touch(),
        _ => {
            eprintln!(
                "usage: alluno-input probe | key <name> | mouse | pad <profile> [seconds] | pen | touch\n\
                 profiles: xbox360 xboxone xboxseries ds4 dualsense switch generic"
            );
            std::process::exit(2);
        }
    };
    if let Err(error) = outcome {
        eprintln!("alluno-input: {error}");
        std::process::exit(1);
    }
}

fn probe() -> alluno_input::Result<()> {
    let caps = alluno_input::probe();
    println!("keyboard   {:?}", caps.keyboard);
    println!("mouse      {:?}", caps.mouse);
    println!("pen        {:?}", caps.pen);
    println!("touch      {:?}", caps.touch);
    for (profile, backing) in &caps.gamepads {
        println!("gamepad    {profile:<12?} {backing:?}");
    }
    if let Some(max) = caps.max_gamepads {
        println!("max pads   {max}");
    }
    println!("midi       {:?}", caps.midi);
    println!("camera     {:?}", caps.camera);
    println!("microphone {:?}", caps.microphone);
    Ok(())
}

fn key(name: Option<&str>) -> alluno_input::Result<()> {
    let key = name.and_then(parse_key).ok_or_else(|| {
        alluno_input::Error::unavailable(
            "unknown key; try a letter, a digit, space, enter or escape",
        )
    })?;
    let host = Runtime::open(Options::default())?;
    let mut keyboard = host.keyboard()?;
    keyboard.key(key, true)?;
    keyboard.key(key, false)?;
    println!("pressed {key:?}");
    Ok(())
}

fn pad(profile: Option<&str>, seconds: Option<&str>) -> alluno_input::Result<()> {
    let profile = match profile {
        Some("xbox360") | None => GamepadProfile::Xbox360,
        Some("xboxone") => GamepadProfile::XboxOne,
        Some("xboxseries") => GamepadProfile::XboxSeries,
        Some("ds4") | Some("dualshock4") => GamepadProfile::DualShock4,
        Some("dualsense") => GamepadProfile::DualSense,
        Some("switch") => GamepadProfile::SwitchPro,
        Some("generic") => GamepadProfile::GenericHid,
        Some(other) => {
            return Err(alluno_input::Error::unavailable(format!(
                "unknown profile {other}"
            )));
        }
    };
    let hold = Duration::from_secs(seconds.and_then(|s| s.parse().ok()).unwrap_or(5));
    let host = Runtime::open(Options::default())?;
    let mut pad = host.gamepad(profile)?;
    pad.on_output(Box::new(|output: GamepadOutput| {
        println!("output {output:?}")
    }))?;
    println!(
        "plugged {profile:?} in slot {:?}, holding A for {hold:?}",
        pad.slot()
    );
    let started = Instant::now();
    while started.elapsed() < hold {
        pad.submit(&GamepadState {
            buttons: buttons::A,
            ..GamepadState::default()
        })?;
        std::thread::sleep(Duration::from_millis(16));
    }
    pad.submit(&GamepadState::default())?;
    Ok(())
}

fn mouse() -> alluno_input::Result<()> {
    let host = Runtime::open(Options::default())?;
    let mut mouse = host.mouse()?;
    for _ in 0..20 {
        mouse.move_rel(4, 0)?;
        std::thread::sleep(Duration::from_millis(8));
    }
    for _ in 0..20 {
        mouse.move_rel(-4, 0)?;
        std::thread::sleep(Duration::from_millis(8));
    }
    mouse.wheel(0, 0)?;
    println!("nudged the cursor right and back");
    Ok(())
}

fn pen() -> alluno_input::Result<()> {
    let host = Runtime::open(Options::default())?;
    let mut pen = host.pen()?;
    let mut sample = PenState {
        x: 32768,
        y: 32768,
        in_range: true,
        ..PenState::default()
    };
    pen.report(&sample)?;
    std::thread::sleep(Duration::from_millis(100));
    sample.down = true;
    sample.pressure = 30000;
    for step in 0..30u16 {
        sample.x = 32768 + step * 200;
        pen.report(&sample)?;
        std::thread::sleep(Duration::from_millis(16));
    }
    sample.down = false;
    sample.pressure = 0;
    pen.report(&sample)?;
    println!("drew a short stroke from the centre");
    Ok(())
}

fn touch() -> alluno_input::Result<()> {
    let host = Runtime::open(Options::default())?;
    let mut touch = host.touch()?;
    let mut contact = TouchContact {
        id: 1,
        x: 32768,
        y: 32768,
        pressure: 30000,
        width: 600,
        height: 600,
        down: true,
    };
    for step in 0..30u16 {
        contact.y = 32768 + step * 200;
        touch.report(&TouchState {
            contacts: vec![contact],
        })?;
        std::thread::sleep(Duration::from_millis(16));
    }
    contact.down = false;
    touch.report(&TouchState {
        contacts: vec![contact],
    })?;
    println!("dragged one finger down from the centre");
    Ok(())
}

fn parse_key(name: &str) -> Option<Key> {
    let lower = name.to_ascii_lowercase();
    let mut chars = lower.chars();
    let first = chars.next()?;
    if chars.next().is_none() {
        let index = match first {
            'a'..='z' => (first as u8 - b'a') as usize,
            '0'..='9' => 26 + (first as u8 - b'0') as usize,
            _ => return None,
        };
        return Key::ALL.get(index).copied();
    }
    Some(match lower.as_str() {
        "space" => Key::Space,
        "enter" => Key::Enter,
        "escape" | "esc" => Key::Escape,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        _ => return None,
    })
}
