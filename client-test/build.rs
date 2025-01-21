use std::{env, fs::{read_to_string, write}, path::{Path, PathBuf}, process::Command};
use wit_component::ComponentEncoder;


fn main() {
    println!("cargo:rerun-if-changed=../../");
    build_and_compose_test_component();
    // build_and_generate_tests();
}



fn build_and_compose_test_component() {
    let meta = cargo_metadata::MetadataCommand::new().exec().unwrap();
    let targets = meta
        .packages
        .iter()
        .find(|p| p.name == "client-test")
        .unwrap()
        .metadata
        .as_object()
        .unwrap()
        .get("runnercomponent")
        .unwrap()
        .as_object(). unwrap();
        
    
    for (key, path) in targets.into_iter() {
        
        // build component
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        let mut cmd = Command::new("cargo-component");
        cmd.arg("build")
            .arg(format!("--package={}",&key))
            .env("CARGO_TARGET_DIR", &out_dir)
            .env("CARGO_PROFILE_DEV_DEBUG", "1");
            println!("running: {cmd:?}");
            let status = cmd.status().unwrap();
            assert!(status.success());

        compose_component(&key);

        let mut wit_world = Vec::new();
        wit_world.push("wasmtime::component::bindgen!({\n".to_string());
        wit_world.push("inline: \"".to_string());
        let wit_path = path.as_object().unwrap().get("path").unwrap().as_str().unwrap();
        println!("hello wit{}",wit_path);

        for line in read_to_string(wit_path).unwrap().lines() {
            if line.contains("import") {
                continue;
            }
            if line.contains("///") {
                continue;
            }
            wit_world.push(line.to_string());
            wit_world.push("\n".to_string());
        }

        wit_world.push("\"});".to_string());
        let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(format!("{}_WIT.rs", &key));
        write(out_dir, wit_world.join("")).unwrap();
    }
   
    println!("done with generating build details");
       
}



fn compose_component(package_name: &str) -> PathBuf {
    println!("package name is {}", package_name);
    let meta = cargo_metadata::MetadataCommand::new().exec().unwrap();
    let targets = meta
            .packages
            .iter()
            .find(|p| p.name == package_name)
            .unwrap()
            .metadata
            .as_object()
            .unwrap();
    println!("target  is {:?}", targets.clone());
    let real_targets = targets.get("component")
        .unwrap()
        .as_object()
        .unwrap()
        .get("target")
        .unwrap()
        .as_object()
        .unwrap()
        .get("dependencies").unwrap()
        .as_object().unwrap();

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let mut cmd = Command::new("cargo-component");
    cmd.arg("build")
        .arg(format!("--package={}",package_name))
        .env("CARGO_TARGET_DIR", &out_dir)
        .env("CARGO_PROFILE_DEV_DEBUG", "1");
        println!("running: {cmd:?}");
        let status = cmd.status().unwrap();
        assert!(status.success());
    
    let built_path = out_dir
        .join("wasm32-wasi")
        .join("debug")
        .join(format!("{}.wasm",package_name));
    
    if real_targets.is_empty() {
        return  built_path;    
    }


    let mut wac = Command::new("wac");
    let artifact_path = built_path.to_str().unwrap();
    wac.arg("plug")
    .arg(format!("{artifact_path}"));


    for (key, path) in real_targets.into_iter() {
        let modified_key = key.split(":").collect::<Vec<&str>>()[1];
        let path  = compose_component(modified_key); 
        println!("this is output path{:?}", path);
        wac.arg("--plug")
        .arg(format!("{}",path.to_str().unwrap()));
    }

    let output_path = out_dir
        .join("wasm32-wasi")
        .join("debug")
        .join(format!("{}-composed.wasm",package_name));
    wac.arg("-o")
    .arg(format!("{}",output_path.to_str().unwrap()));
    let status = wac.status().unwrap();
    assert!(status.success());

    return  output_path;

}

