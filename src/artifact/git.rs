// #[allow(clippy::expect_fun_call)]
// pub fn clone_repo(
//     &self,
//     contract: &Contract,
//     artifact_folder: &str,
// ) -> Result<(), git2::Error> {
//     let url = contract.url.clone();
//     let git_name = url.split('/').collect::<Vec<&str>>().pop().unwrap();
//     let repo_name = git_name.replace(".git", "");
//     let path = format!(
//         "{}/{}/{}",
//         artifact_folder, DEFAULT_PROJECTS_FOLDER, repo_name
//     );

//     if Path::new(&path).is_dir() {
//         println!("Repository already exist: {}", repo_name);
//         Repository::open(&path).expect(format!("Cant open repo [{}]", repo_name).as_str());
//         Ok(())
//     } else {
//         let mut cb = git2::RemoteCallbacks::new();
//         let git_config = git2::Config::open_default().unwrap();
//         let mut ch = CredentialHandler::new(git_config);
//         cb.credentials(move |url, username, allowed| {
//             ch.try_next_credential(url, username, allowed)
//         });

//         // clone a repository
//         let mut fo = git2::FetchOptions::new();
//         fo.remote_callbacks(cb)
//             .download_tags(git2::AutotagOption::All)
//             .update_fetchhead(true);
//         println!("Cloning repo: {} on: {}", url, path);
//         git2::build::RepoBuilder::new()
//             .branch(&contract.branch)
//             .fetch_options(fo)
//             .clone(&url, path.as_ref())
//             .expect(format!("Cant clone repo [{}]", repo_name).as_str());
//         Ok(())
//     }
// }

// fn wasm_compile(&self, contract: &Contract, artifact_folder: &str) -> Result<(), io::Error> {
//     let url = contract.url.clone();
//     let git_name = url.split('/').collect::<Vec<&str>>().pop().unwrap();
//     let repo_name = git_name.replace(".git", "");
//     let cargo_path = &contract.cargo_path;
//     let path = format!(
//         "{}/{}/{}/{}",
//         artifact_folder, DEFAULT_PROJECTS_FOLDER, repo_name, cargo_path
//     );

//     if Path::new(&path).is_dir() {
//         //let command = format!("cargo cw-optimizoor {}/Cargo.toml", path);
//         // Note: https://github.com/mandrean/cw-optimizoor/blob/87fbbcea67398dfa9cb21165848b7448d98f17c4/src/lib.rs has some problems with workspaces
//         println!(
//             "current dir[{}] project dir[{}]",
//             std::env::current_dir().unwrap().to_str().unwrap(),
//             path
//         );
//         let command = format!(
//             "(cd {}; cargo build --release --locked --target wasm32-unknown-unknown --lib)",
//             path
//         );
//         println!("Command [{}]", command);
//         let status = Command::new("bash")
//             .arg("-c")
//             .arg(command)
//             .stdout(Stdio::inherit())
//             .status()
//             .expect("cargo build failed");

//         println!("process finished with: {:#?}", status);

//         println!(
//             "Artifacts generated on {}/target/wasm32-unknown-unknown/debug/",
//             path
//         );

//         for entry in fs::read_dir(format!(
//             "{}/target/wasm32-unknown-unknown/release/deps/",
//             path
//         ))
//         .unwrap()
//         {
//             let path = entry.as_ref().unwrap().path();
//             if let Some(extension) = path.extension() {
//                 if extension == "wasm" {
//                     let filename = entry.as_ref().unwrap().file_name().into_string().unwrap();
//                     if contract.artifact.eq(&filename) {
//                         rename(
//                             path,
//                             format!(
//                                 "{}/{}",
//                                 artifact_folder,
//                                 entry.as_ref().unwrap().file_name().into_string().unwrap()
//                             ),
//                         )
//                         .expect("Failed renaming wasm files");
//                     }
//                 }
//             }
//         }
//     } else {
//         println!("Path to compile doesn't exist [{}]", path);
//     }
//     Ok(())
// }
