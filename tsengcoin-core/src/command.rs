use std::{cmp::max, collections::HashMap, error::Error};

pub struct Command<T> {
    pub processor: CommandProcessor<T>,
    pub expected_fields: Vec<Field>,
    pub flags: Vec<Flag>,
    pub optionals: Vec<VarField>,
    pub desc: String,
}

/// The function that actually executes the command. Accepts the parameters passed into the command,
/// and the state/context object
pub type CommandProcessor<T> =
    fn(invocation: &CommandInvocation, state: Option<T>) -> Result<(), Box<dyn Error>>;
pub type CommandMap<T> = HashMap<String, Command<T>>;

pub struct CommandInvocation {
    /// The name of the command that was invoked
    pub name: String,

    /// Boolean flags passed to the command wtih '--flag' syntax
    pub flags: Vec<String>,

    /// Ordered args passed to the command with '--' args removed
    pub args: Vec<String>,

    /// Args passed to the command with '--field=value' syntax
    pub vars: HashMap<String, String>,

    /// You probably want this: this contains values for expected args.
    /// Expected args are passed differently depending on their `FieldType`
    pub fields: HashMap<String, String>,

    /// Optional arguments, set as variables
    pub optionals: HashMap<String, String>,
}

pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub desc: String,
    /// A condition can specify the name of a flag that can disable this field. If the flag is present, the field will not be required.
    pub condition: Option<Condition>,
}

#[derive(Clone)]
pub struct Flag {
    pub name: String,
    pub desc: String,
}

#[derive(Clone)]
pub struct VarField {
    pub name: String,
    pub desc: String,
    pub placeholder: Option<String>,
}

#[derive(PartialEq)]
pub enum FieldType {
    /// A "var" must be passed in as a named variable with --name=value syntax
    Var,

    /// A pos argument is expected to be found at the given position in the args vector
    /// if not passed in as a var
    Pos(usize),

    /// A spaces argument, if not passed in as a var, is expected to be found starting at the given
    /// position. The argument consists of all tokens after and including the one at the given position,
    /// joined by spaces. A spaces argument should only ever be the last argument expected.
    Spaces(usize),
}

pub struct Condition {
    pub disable_flag: String,
    pub desc: String,
}

impl CommandInvocation {
    pub fn get_flag(&self, flag: &str) -> bool {
        self.flags.contains(&flag.to_owned())
    }

    pub fn get_field(&self, field_name: &str) -> Option<String> {
        self.fields.get(field_name).cloned()
    }

    pub fn get_optional(&self, field_name: &str) -> Option<String> {
        self.optionals.get(field_name).cloned()
    }
}

impl Field {
    pub fn new(name: &str, field_type: FieldType, desc: &str) -> Self {
        Field {
            name: name.to_owned(),
            field_type,
            desc: desc.to_owned(),
            condition: None,
        }
    }

    pub fn new_condition(
        name: &str,
        field_type: FieldType,
        desc: &str,
        condition: Condition,
    ) -> Self {
        Field {
            name: name.to_owned(),
            field_type,
            desc: desc.to_owned(),
            condition: Some(condition),
        }
    }
}

impl Flag {
    pub fn new(name: &str, desc: &str) -> Self {
        Flag {
            name: name.to_owned(),
            desc: desc.to_owned(),
        }
    }
}

impl VarField {
    pub fn new(name: &str, desc: &str) -> Self {
        Self {
            name: name.to_owned(),
            desc: desc.to_owned(),
            placeholder: None,
        }
    }

    pub fn new_placeholder(name: &str, desc: &str, placeholder: &str) -> Self {
        Self {
            name: name.to_owned(),
            desc: desc.to_owned(),
            placeholder: Some(placeholder.to_owned()),
        }
    }
}

impl Condition {
    pub fn new(flag: &str, desc: &str) -> Self {
        Condition {
            disable_flag: flag.to_owned(),
            desc: desc.to_owned(),
        }
    }
}

pub fn dispatch_command<T>(args: &Vec<String>, map: &CommandMap<T>, state: Option<T>) {
    if args.is_empty() {
        println!("Missing command");
        return;
    }

    let cmd_name = &args[0];

    if cmd_name == "help" {
        if args.len() < 2 {
            help(map);
        } else {
            let cmd_name = args[1].clone();
            help_cmd(map, cmd_name);
        }

        return;
    }

    let command = match map.get(cmd_name) {
        Some(obj) => obj.to_owned(),
        None => {
            println!("Unrecognized command: {cmd_name}");
            return;
        }
    };

    let invocation = match decompose_raw_args(args, &command.expected_fields, &command.optionals) {
        Err(err) => {
            eprintln!("Failed to decompose command: {}", err);
            return;
        }
        Ok(inv) => inv,
    };

    match (command.processor)(&invocation, state) {
        Err(err) => eprintln!("Error executing command: {}", err),
        Ok(_) => (),
    }
}

fn decompose_raw_args(
    raw_args: &Vec<String>,
    expected_fields: &Vec<Field>,
    possible_optionals: &Vec<VarField>,
) -> Result<CommandInvocation, Box<dyn Error>> {
    let cmd_name = &raw_args[0];
    let trimmed_args = &raw_args[1..];
    let mut assignments: HashMap<String, String> = HashMap::new();
    let mut optionals: HashMap<String, String> = HashMap::new();
    let (specials, ordered_args): (Vec<String>, Vec<String>) = trimmed_args
        .iter()
        .map(|s| s.to_owned())
        .partition(|s| s.starts_with("--"));

    let (assignment_strs, flags): (Vec<String>, Vec<String>) = specials
        .iter()
        .map(|s| s.trim_start_matches("--").to_owned())
        .partition(|s| s.contains('='));

    for assignment in assignment_strs {
        let pair: Vec<&str> = assignment.split('=').collect();
        let key = pair[0].to_owned();
        let value = pair[1].to_owned();

        if possible_optionals.iter().any(|f| f.name == key) {
            optionals.insert(key, value);
        } else {
            assignments.insert(key, value);
        }
    }

    let mut fields: HashMap<String, String> = HashMap::new();
    let mut pos_fields: Vec<Field> = vec![];

    // Process explicitly assigned fields first and recalculate positions for
    // positional fields
    for Field {
        name,
        field_type,
        desc,
        condition,
    } in expected_fields
    {
        // Will only be Some if the field was assigned with `--name=value` syntax
        let var_field = assignments.get(name).cloned();

        if condition.is_some() {
            let cond = condition.as_ref().unwrap();

            if flags.contains(&cond.disable_flag) {
                continue;
            }
        }

        match (field_type, var_field) {
            (FieldType::Var, Some(var)) => drop(fields.insert(name.to_owned(), var)),
            (FieldType::Var, None) => {
                return Err(format!(
                    "Missing expected argument {name}. Pass this in with --{name}=<value>"
                )
                .into())
            }
            (FieldType::Pos(_) | FieldType::Spaces(_), Some(var)) => {
                drop(fields.insert(name.to_owned(), var))
            }
            (FieldType::Pos(_), None) => {
                pos_fields.push(Field::new(name, FieldType::Pos(pos_fields.len()), desc))
            }
            (FieldType::Spaces(_), None) => {
                pos_fields.push(Field::new(name, FieldType::Spaces(pos_fields.len()), desc))
            }
        }
    }

    // Now go through the remaining ordered arguments with new positions and pick them out
    for Field {
        name, field_type, ..
    } in pos_fields
    {
        match field_type {
            FieldType::Var => unreachable!(),
            FieldType::Pos(pos) if pos < ordered_args.len() => {
                drop(fields.insert(name.to_owned(), ordered_args[pos.to_owned()].clone()))
            }
            FieldType::Spaces(pos) if pos < ordered_args.len() => {
                drop(fields.insert(name.to_owned(), ordered_args[pos.to_owned()..].join(" ")))
            }
            _ => {
                return Err(
                    format!("Not enough arguments: missing expected argument {name}").into(),
                )
            }
        };
    }

    let out = CommandInvocation {
        name: cmd_name.to_owned(),
        flags,
        args: ordered_args,
        vars: assignments,
        fields,
        optionals,
    };

    Ok(out)
}

fn help<T>(map: &CommandMap<T>) {
    println!("These are the supported commands. Do 'help command_name' to learn more about a specific command.\n");
    let mut keys: Vec<String> = map.keys().map(|k| k.to_owned()).collect();
    keys.sort();

    for cmd_name in keys {
        let cmd = map.get(&cmd_name).unwrap();
        println!("\t{}\n\t\t{}", cmd_name, cmd.desc);
    }
}

fn help_cmd<T>(map: &CommandMap<T>, cmd_name: String) {
    let command = match map.get(&cmd_name) {
        Some(obj) => obj.to_owned(),
        None => {
            println!("Unrecognized command: {cmd_name}");
            return;
        }
    };

    println!("{}\n", command.desc);
    println!("Syntax: \t{}", make_syntax_string(&cmd_name, command));

    let (vars, mut poses): (Vec<&Field>, Vec<&Field>) = command
        .expected_fields
        .iter()
        .partition(|f| f.field_type == FieldType::Var);

    poses.sort_by_key(|f| match f.field_type {
        FieldType::Pos(pos) => pos,
        FieldType::Spaces(pos) => pos,
        _ => unreachable!(),
    });

    let cond_fields = poses
        .iter()
        .filter(|f| f.condition.is_some())
        .copied()
        .collect::<Vec<&Field>>();

    let mut var_names: Vec<(String, String)> = vars
        .iter()
        .map(|f| (f.name.to_owned(), f.desc.to_owned()))
        .collect();

    var_names.sort();

    let mut flags = command.flags.clone();
    flags.sort_by_key(|f| f.name.clone());

    if !poses.is_empty() {
        println!("\nRequired arguments:\n");

        for field in poses {
            println!("\t{}\n\t\t{}", field.name, field.desc)
        }
    } else {
        println!("\nThere are no required positional arguments");
    }

    if !var_names.is_empty() {
        println!("\nRequired keyword arguments:\n");

        for (name, desc) in var_names {
            println!("\t--{name}\n\t\t{desc}");
        }
    }

    if !flags.is_empty() {
        println!("\nOptional flags:\n");

        for Flag { name, desc } in flags {
            println!("\t--{name}\n\t\t{desc}");
        }

        for field in cond_fields {
            let cond = field.condition.as_ref().unwrap();
            println!(
                "\t--{} (instead of <{}>)\n\t\t{}",
                cond.disable_flag, field.name, cond.desc
            );
        }
    }

    if !command.optionals.is_empty() {
        println!("\nOptional arguments:\n");

        for VarField { name, desc, .. } in &command.optionals {
            println!("\t--{name}\n\t\t{desc}");
        }
    }
}

fn make_syntax_string<T>(name: &String, command: &Command<T>) -> String {
    let mut out = String::from(name);
    let mut max_pos: isize = -1;

    for field in &command.expected_fields {
        match field.field_type {
            FieldType::Pos(pos) => max_pos = max(max_pos, pos.to_owned().try_into().unwrap()),
            FieldType::Spaces(pos) => max_pos = max(max_pos, pos.to_owned().try_into().unwrap()),
            FieldType::Var => (),
        };
    }

    for field in &command.optionals {
        let placeholder = match &field.placeholder {
            None => String::from("value"),
            Some(placeholder) => placeholder.clone(),
        };

        out.push(' ');
        out.push_str(&format!("[--{}={}]", field.name, placeholder));
    }

    let mut names: Vec<String> = Vec::with_capacity((max_pos + 1).try_into().unwrap());
    for _ in 0..names.capacity() {
        names.push(String::from(""));
    }

    for Field {
        name,
        field_type,
        desc: _,
        condition,
    } in &command.expected_fields
    {
        let format_name = match condition {
            None => format!("<{}>", name),
            Some(cond) => format!("<--{}|{}>", cond.disable_flag, name),
        };

        match field_type {
            FieldType::Pos(pos) => names[pos.to_owned()] = format_name,
            FieldType::Spaces(pos) => names[pos.to_owned()] = format_name,
            FieldType::Var => (),
        };
    }

    for name in names {
        out.push(' ');
        out.push_str(&name);
    }

    out
}
