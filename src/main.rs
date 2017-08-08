extern crate gtk;
extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

use std::process::Command;
use std::fs::File;
use gtk::prelude::*;
use gtk::{Entry, Label, Window, ScrolledWindow, WindowType, Box, Orientation};

const KEY_ESCAPE: u32 = 65307;
const KEY_ENTER : u32 = 65293;

#[derive(Deserialize, PartialEq, Eq, Debug, Clone)]
struct Node {
    shortcut: String,
    description: String,
    command: Option<String>,
    children: Option<Vec<Node>>,
}

#[derive(Deserialize, Clone)]
struct Config {
    shell_prefix: String,
    menu: Vec<Node>,
}

fn borrow_nodes(nodes: &Vec<Node>) -> Vec<&Node> {
    let mut ret = Vec::new();

    for n in nodes {
        ret.push(n);
    }

    ret
}

#[derive(PartialEq, Eq, Debug)]
enum Resolved<'a> {
    Partial(Vec<&'a Node>),
    Complete(&'a Node)
}

fn resolve<'a>(nodes: Vec<&'a Node>, command: String) -> Resolved {
    if command.len() == 0 {
        return Resolved::Partial(nodes);
    }

    let mut partial = vec![];

    for n in nodes {
        if n.shortcut.as_str().starts_with(command.as_str()) {
            partial.push(n);
        }

        if !command.as_str().starts_with(n.shortcut.as_str()) {
            continue;
        }

        let remaining: String = command.chars().skip(n.shortcut.len()).
            collect();

        if remaining.len() == 0 && n.children == None {
            return Resolved::Complete(n);
        }

        if remaining.len() == 0 {
            return match n.children {
                Some(ref c) => Resolved::Partial(borrow_nodes(c)),
                None        => Resolved::Complete(n),
            };
        }

        return match n.children {
            Some(ref c) => resolve(borrow_nodes(c), remaining),
            None        => Resolved::Partial(vec![]),
        };
    }

    Resolved::Partial(partial)
}

fn clear_output(output: &Box) {
    let labels = output.get_children();

    for label in labels {
        label.destroy();
    }
}

fn set_output_nodes(output: &Box, nodes: Vec<&Node>) {
    clear_output(output);

    for node in nodes {
        let outer = Box::new(Orientation::Horizontal, 0);
        output.add(&outer);

        let shortcut_text: &str = &node.shortcut;
        let shortcut = Label::new(shortcut_text);
        outer.add(&shortcut);

        let description_text: &str = &node.description;
        let description = Label::new(description_text);
        description.set_alignment(1.0, 0.0);
        description.set_hexpand(true);
        outer.add(&description);
    }

    output.show_all();
}

fn set_output_text(output: &Box, text: &str) {
    clear_output(output);

    let label = Label::new(text);
    output.add(&label);

    output.show_all();
}

fn main() {
    let config_file = File::open("/etc/blaunch.json").
        expect("Can't open /etc/blaunch.json");

    let config: Config = match serde_json::from_reader(config_file) {
        Ok(n)  => n,
        Err(e) => panic!("Can't parse /etc/blaunch.json: {}", e),
    };

    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let window = Window::new(WindowType::Toplevel);
    window.set_title("blaunch");
    window.set_default_size(350, 200);

    let vbox = Box::new(Orientation::Vertical, 0);
    window.add(&vbox);

    let command = Entry::new();
    vbox.add(&command);

    let scrolled = ScrolledWindow::new(None, None);
    scrolled.set_vexpand(true);
    vbox.add(&scrolled);

    let output_lines = Box::new(Orientation::Vertical, 0);
    scrolled.add(&output_lines);

    set_output_nodes(&output_lines, borrow_nodes(&config.menu));

    command.grab_focus();

    window.show_all();
    window.connect_delete_event(|_, _| {
        gtk::main_quit();
        Inhibit(false)
    });

    let c_config = config.clone();
    command.connect_changed(move |c| {
        let value = c.get_text().unwrap_or("".to_string());

        // Handle shell prefix
        if value.starts_with(&c_config.shell_prefix) {
            set_output_text(&output_lines, "Enter a shell command..");
            return;
        }

        // Handle menu matching
        match resolve(borrow_nodes(&c_config.menu), value) {
            Resolved::Complete(n) => {
                let command = match n.command {
                    Some(ref c) => c,
                    None        => panic!("No command for {}", n.shortcut),
                };

                match Command::new(command).spawn() {
                    Ok (_) => {},
                    Err(e) => panic!("Can't start process: {}", e),
                };

                gtk::main_quit();
            },
            Resolved::Partial(nodes) => {
                set_output_nodes(&output_lines, nodes);
            },
        };
    });

    let kp_config = config.clone();
    command.connect_key_press_event(move |c, e| {
        if e.get_keyval() == KEY_ESCAPE {
            gtk::main_quit();
        }

        if e.get_keyval() == KEY_ENTER {
            let value = c.get_text().unwrap_or("".to_string());

            if value.starts_with(&kp_config.shell_prefix) {
                let command: String = value.chars().skip(
                        kp_config.shell_prefix.len()).collect();

                match Command::new("sh").arg("-c").arg(command).spawn() {
                    Ok (_) => gtk::main_quit(),
                    Err(e) => panic!("Can't start process: {}", e),
                };
            }
        }

        Inhibit(false)
    });

    gtk::main();
}

#[cfg(test)]
mod tests {
    use super::{Node, Resolved, resolve, borrow_nodes};

    fn test_data() -> Vec<Node> {
        vec![Node {
            shortcut: "terminal".to_string(),
            description: "terminal emulator".to_string(),
            command: Some("xfce4-terminal".to_string()),
            children: None,
        }, Node {
            shortcut: "web".to_string(),
            description: "web browsers".to_string(),
            command: None,
            children: Some(vec![Node {
                shortcut: "chrome".to_string(),
                description: "Google Chrome".to_string(),
                command: Some("chromium".to_string()),
                children: None,
            }, Node {
                shortcut: "firefox".to_string(),
                description: "Mozilla FireFox".to_string(),
                command: Some("firefox".to_string()),
                children: None,
            }]),
        }]
    }

    fn expect_partial(command: &str, mut expected: Vec<&str>) {
        let data = test_data();
        let nodes = match resolve(borrow_nodes(&data), command.to_string()) {
            Resolved::Complete(_) => panic!("Expected partial match"),
            Resolved::Partial(n)  => n,
        };

        for node in nodes {
            let p = match expected.iter().position(|&e| e == node.shortcut) {
                None    => panic!("Unexpected node {}", node.shortcut),
                Some(p) => p,
            };

            expected.remove(p);
        }

        if expected.len() != 0 {
            panic!("Expected nodes missing: {:?}", expected);
        }
    }

    fn expect_complete(command: &str, expected: &str) {
        let data = test_data();
        let node = match resolve(borrow_nodes(&data), command.to_string()) {
            Resolved::Partial(_)  => panic!("Expected complete match"),
            Resolved::Complete(n) => n,
        };

        assert_eq!(node.shortcut, expected);
    }

    #[test]
    fn it_resolves_no_match_to_empty() {
        expect_partial("wrong", vec![]);
    }

    #[test]
    fn it_resolves_empty_string_to_root_node() {
        expect_partial("", vec!["web", "terminal"]);
    }

    #[test]
    fn it_resolves_partial_first_level() {
        expect_partial("t", vec!["terminal"]);
    }

    #[test]
    fn it_resolves_second_level() {
        expect_partial("web", vec!["firefox", "chrome"]);
    }

    #[test]
    fn it_resolves_partial_second_level() {
        expect_partial("webchr", vec!["chrome"]);
    }

    #[test]
    fn it_resolves_complete_first_level() {
        expect_complete("terminal", "terminal");
    }

    #[test]
    fn it_resolves_complete_second_level() {
        expect_complete("webfirefox", "firefox");
    }
}

