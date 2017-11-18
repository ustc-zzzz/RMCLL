#![allow(dead_code)]

use std::path;
use std::result::Result;
use std::collections::HashMap;
use std::process::{Child, Command};

use parsing;
use versions;
use yggdrasil;

#[derive(Debug)]
pub struct JvmOption(String);

#[derive(Debug)]
pub struct GameOption(String, Option<String>);

pub struct MinecraftLauncher {
    version_id: String,
    program_path: String,
    game_root_dir: path::PathBuf,
    assets_dir: path::PathBuf,
    libraries_dir: path::PathBuf,
    manager: versions::VersionManager,
    launcher_name_version: (String, String),
    auth_info: yggdrasil::AuthInfo,
    window_resolution: (u32, u32),
}

#[derive(Debug)]
pub struct LaunchArguments {
    java_main_class: String,
    java_program_path: String,
    jvm_options: Vec<JvmOption>,
    game_options: Vec<GameOption>,
    game_native_path: path::PathBuf,
    game_natives: versions::NativeCollection,
}

pub fn create(game_dir: path::PathBuf,
              game_version_id: &str,
              game_auth_info: yggdrasil::AuthInfo) -> MinecraftLauncher {
    MinecraftLauncher {
        version_id: game_version_id.to_owned(),
        program_path: find_jre().pop().expect("Java Runtime Environment not found"),
        assets_dir: game_dir.as_path().join("assets/"),
        libraries_dir: game_dir.as_path().join("libraries/"),
        manager: versions::VersionManager::new(game_dir.as_path().join("versions/").as_path()),
        game_root_dir: game_dir,
        launcher_name_version: ("RMCLL".to_owned(), "0.1.0".to_owned()),
        auth_info: game_auth_info,
        window_resolution: (854, 480),
    }
}

#[cfg(target_os = "windows")]
pub fn find_jre() -> Vec<String> {
    Vec::new() // TODO
}

#[cfg(target_os = "macos")]
pub fn find_jre() -> Vec<String> {
    Vec::new() // TODO: I cannot afford a mac
}

#[cfg(target_os = "linux")]
pub fn find_jre() -> Vec<String> {
    let program = "update-alternatives";
    if let Result::Ok(output) = Command::new(program).arg("--list").arg("java").output() {
        if let Result::Ok(string) = String::from_utf8(output.stdout) {
            return string.trim().split_whitespace().map(String::from).collect();
        }
    }
    let program = "which";
    if let Result::Ok(output) = Command::new(program).arg("java").output() {
        if let Result::Ok(string) = String::from_utf8(output.stdout) {
            return vec![String::from(string.trim())];
        }
    }
    Vec::new()
}

impl MinecraftLauncher {
    pub fn generate_argument_map(&self,
                                 version: &versions::MinecraftVersion) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = HashMap::new();
        let name = self.auth_info.user_profile().name();
        let uuid = self.auth_info.user_profile().uuid().simple();
        let access_token = self.auth_info.access_token().simple();
        map.insert("auth_access_token".to_owned(),
                   format!("{}", access_token));
        map.insert("user_properties".to_owned(),
                   "{}".to_owned()); // TODO
        map.insert("user_property_map".to_owned(),
                   "{}".to_owned()); // TODO
        map.insert("auth_session".to_owned(),
                   format!("token:{}:{}", access_token, uuid));
        map.insert("auth_player_name".to_owned(),
                   name.clone());
        map.insert("auth_uuid".to_owned(),
                   format!("{}", uuid));
        map.insert("user_type".to_owned(),
                   "legacy".to_owned());
        map.insert("profile_name".to_owned(),
                   name.clone());
        map.insert("version_name".to_owned(),
                   self.version_id.clone());
        map.insert("game_directory".to_owned(),
                   self.game_root_dir.to_str().unwrap_or("").to_owned());
        map.insert("assets_root".to_owned(),
                   self.assets_dir.to_str().unwrap_or("").to_owned());
        map.insert("assets_index_name".to_owned(),
                   version.asset_index(&self.manager).map(|i| i.id().to_owned()).unwrap_or_else(String::new));
        map.insert("version_type".to_owned(),
                   version.version_type().to_owned());
        map.insert("resolution_width".to_owned(),
                   format!("{}", self.window_resolution.0));
        map.insert("resolution_height".to_owned(),
                   format!("{}", self.window_resolution.1));
        map.insert("language".to_owned(),
                   "en-us".to_owned());
        map.insert("launcher_name".to_owned(),
                   self.launcher_name_version.0.clone());
        map.insert("launcher_version".to_owned(),
                   self.launcher_name_version.1.clone());
        map.insert("natives_directory".to_owned(),
                   self.manager.get_natives_path(&self.version_id).to_str().unwrap_or("").to_owned());
        map.insert("primary_jar".to_owned(),
                   self.manager.get_primary_jar_path(&self.version_id).to_str().unwrap_or("").to_owned());
        map.insert("classpath".to_owned(),
                   version.classpath(self.libraries_dir.as_path(), &self.manager).unwrap_or_else(|_| String::new()));
        map.insert("classpath_separator".to_owned(),
                   ":".to_owned());
        map
    }

    pub fn to_launch_arguments(&self) -> Result<LaunchArguments, versions::Error> {
        let java_program_path = self.program_path.clone();
        let minecraft_version = self.manager.version_of(&self.version_id)?;
        let java_main_class = minecraft_version.main_class(&self.manager).unwrap_or_else(String::new);
        let game_natives = minecraft_version.to_native_collection(&self.manager, self.libraries_dir.as_path())?;
        let mut jvm_options = vec![
            JvmOption::new("-Xmn128m".to_owned()),
            JvmOption::new("-Xmx2048m".to_owned()),
            JvmOption::new("-XX:+UseG1GC".to_owned()),
            JvmOption::new("-XX:-UseAdaptiveSizePolicy".to_owned()),
            JvmOption::new("-XX:-OmitStackTraceInFastThrow".to_owned()),
            JvmOption::new("-Dfml.ignoreInvalidMinecraftCertificates=true".to_owned()),
            JvmOption::new("-Dfml.ignorePatchDiscrepancies=true".to_owned()),
        ];
        let mut game_options = Vec::new();
        let map = self.generate_argument_map(&minecraft_version);
        let game_native_path = path::PathBuf::from(map.get("natives_directory").unwrap());
        let strategy = parsing::ParameterStrategy::map(move |s| {
            let result = match map.get(&s) {
                Some(ref string) => (*string).clone(),
                None => String::new()
            };
            result
        });
        minecraft_version.collect_game_arguments(&self.manager, &mut game_options, &strategy)?;
        minecraft_version.collect_jvm_arguments(&self.manager, &mut jvm_options, &strategy)?;
        Result::Ok(LaunchArguments {
            game_natives,
            game_native_path,
            game_options,
            jvm_options,
            java_main_class,
            java_program_path,
        })
    }
}

impl LaunchArguments {
    pub fn start(&self) -> Result<Child, versions::Error> {
        self.extract_natives()?;
        self.spawn_new_process()
    }

    pub fn spawn_new_process(&self) -> Result<Child, versions::Error> {
        Command::new(self.program()).args(self.args()).spawn().map_err(versions::Error::from)
    }

    pub fn extract_natives(&self) -> Result<Vec<String>, versions::Error> {
        self.game_natives.extract_to(self.game_native_path.as_path())
    }

    pub fn program(&self) -> String {
        self.java_program_path.clone()
    }

    pub fn args(&self) -> Vec<String> {
        let mut result = Vec::new();
        for option in self.jvm_options.iter() {
            match option {
                &JvmOption(ref name) => {
                    result.push(name.clone());
                }
            }
        }
        result.push(self.java_main_class.clone());
        for option in self.game_options.iter() {
            match option {
                &GameOption(ref name, Some(ref arg)) => {
                    result.push(name.clone());
                    result.push(arg.clone());
                }
                &GameOption(ref name, None) => {
                    result.push(name.clone());
                }
            }
        }
        result
    }
}

impl JvmOption {
    pub fn new(arg: String) -> JvmOption {
        JvmOption(arg)
    }
}

impl GameOption {
    pub fn new_pair(name: String, arg: String) -> GameOption {
        GameOption(name, Some(arg))
    }

    pub fn new_single(name: String) -> GameOption {
        GameOption(name, None)
    }
}