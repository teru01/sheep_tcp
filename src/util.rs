use std::collections::HashMap;
use std::fs;

pub fn load_env() -> HashMap<String, String> {
	let contents = fs::read_to_string(".env").expect("Failed to read env file");
	let lines: Vec<_> = contents.split('\n').collect();
	let mut map = HashMap::new();
	for line in lines {
		let elm: Vec<_> = line.split('=').map(str::trim).collect();
		if elm.len() == 2 {
			map.insert(elm[0].to_string(), elm[1].to_string());
		}
	}
	map
}
