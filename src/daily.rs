use serde_json::{Map, Value};

use crate::{
    cli::{
        CommentCreateArgs, SectionCreateArgs, StoryCreateArgs, StoryUpdateArgs, TagCreateArgs,
        TagUpdateArgs, TaskCreateArgs, TaskUpdateArgs,
    },
    error::{Error, Result},
    input,
};

pub fn task_create(
    args: &TaskCreateArgs,
    workspace_gid: Option<&str>,
    implicit_context: bool,
) -> Result<Value> {
    let mut data = Map::new();
    insert_option(&mut data, "name", args.name.clone());
    insert_option(&mut data, "assignee", args.assignee.clone());
    insert_option(&mut data, "due_on", args.due_on.clone());
    insert_option(&mut data, "due_at", args.due_at.clone());
    insert_option(&mut data, "start_on", args.start_on.clone());
    insert_option(&mut data, "start_at", args.start_at.clone());
    insert_option(&mut data, "parent", args.parent.clone());
    insert_text(
        &mut data,
        "notes",
        args.notes.as_deref(),
        args.notes_file.as_deref(),
    )?;
    insert_text(
        &mut data,
        "html_notes",
        args.html_notes.as_deref(),
        args.html_notes_file.as_deref(),
    )?;
    if let Some(section) = &args.section {
        if args.project.len() != 1 {
            return Err(Error::Usage(
                "--section requires exactly one --project when creating a task".to_owned(),
            ));
        }
        data.insert(
            "memberships".to_owned(),
            serde_json::json!([{
                "project": args.project[0],
                "section": section,
            }]),
        );
    } else if !args.project.is_empty() {
        data.insert("projects".to_owned(), serde_json::json!(args.project));
    }
    if !args.follower.is_empty() {
        data.insert("followers".to_owned(), serde_json::json!(args.follower));
    }
    if !args.tag.is_empty() {
        data.insert("tags".to_owned(), serde_json::json!(args.tag));
    }
    if !args.custom_field.is_empty() {
        data.insert(
            "custom_fields".to_owned(),
            Value::Object(args.custom_field.iter().cloned().collect()),
        );
    }
    let mut value = merge_extra(Value::Object(data), args.data.as_deref())?;
    let object = value.as_object_mut().expect("task data is an object");
    let has_context = object.contains_key("workspace")
        || object.contains_key("projects")
        || object.contains_key("memberships")
        || object.contains_key("parent");
    if !has_context && !implicit_context {
        let workspace_gid = workspace_gid.ok_or_else(|| {
            Error::Config(
                "task creation requires a workspace, project, parent, membership, or configured workspace"
                    .to_owned(),
            )
        })?;
        object.insert(
            "workspace".to_owned(),
            Value::String(workspace_gid.to_owned()),
        );
    }
    Ok(value)
}

pub fn task_update(args: &TaskUpdateArgs) -> Result<Value> {
    let mut data = Map::new();
    insert_option(&mut data, "name", args.name.clone());
    insert_option(&mut data, "assignee", args.assignee.clone());
    if args.clear_assignee {
        data.insert("assignee".to_owned(), Value::Null);
    }
    insert_option(&mut data, "due_on", args.due_on.clone());
    insert_option(&mut data, "due_at", args.due_at.clone());
    if args.clear_due {
        data.insert("due_on".to_owned(), Value::Null);
        data.insert("due_at".to_owned(), Value::Null);
    }
    insert_option(&mut data, "start_on", args.start_on.clone());
    insert_option(&mut data, "start_at", args.start_at.clone());
    if args.clear_start {
        data.insert("start_on".to_owned(), Value::Null);
        data.insert("start_at".to_owned(), Value::Null);
    }
    insert_text(
        &mut data,
        "notes",
        args.notes.as_deref(),
        args.notes_file.as_deref(),
    )?;
    insert_text(
        &mut data,
        "html_notes",
        args.html_notes.as_deref(),
        args.html_notes_file.as_deref(),
    )?;
    if args.clear_notes {
        data.insert("notes".to_owned(), Value::String(String::new()));
    }
    if args.complete {
        data.insert("completed".to_owned(), Value::Bool(true));
    } else if args.incomplete {
        data.insert("completed".to_owned(), Value::Bool(false));
    }
    if !args.set_custom_field.is_empty() || !args.clear_custom_field.is_empty() {
        let mut fields: Map<String, Value> = args.set_custom_field.iter().cloned().collect();
        for gid in &args.clear_custom_field {
            fields.insert(gid.clone(), Value::Null);
        }
        data.insert("custom_fields".to_owned(), Value::Object(fields));
    }
    let value = merge_extra(Value::Object(data), args.data.as_deref())?;
    if value.as_object().is_none_or(Map::is_empty) {
        return Err(Error::Usage(
            "task update requires at least one field to change".to_owned(),
        ));
    }
    Ok(value)
}

pub fn comment_create(args: &CommentCreateArgs) -> Result<Value> {
    let mut data = Map::new();
    insert_text(
        &mut data,
        "text",
        args.text.as_deref(),
        args.text_file.as_deref(),
    )?;
    insert_text(
        &mut data,
        "html_text",
        args.html_text.as_deref(),
        args.html_text_file.as_deref(),
    )?;
    Ok(Value::Object(data))
}

pub fn story_update(args: &StoryUpdateArgs) -> Result<Value> {
    let mut data = Map::new();
    insert_text(
        &mut data,
        "text",
        args.text.as_deref(),
        args.text_file.as_deref(),
    )?;
    insert_text(
        &mut data,
        "html_text",
        args.html_text.as_deref(),
        args.html_text_file.as_deref(),
    )?;
    if args.pin {
        data.insert("is_pinned".to_owned(), Value::Bool(true));
    } else if args.unpin {
        data.insert("is_pinned".to_owned(), Value::Bool(false));
    }
    let value = merge_extra(Value::Object(data), args.data.as_deref())?;
    if value.as_object().is_none_or(Map::is_empty) {
        return Err(Error::Usage(
            "story update requires text, pin state, or --data".to_owned(),
        ));
    }
    Ok(value)
}

pub fn story_create(args: &StoryCreateArgs) -> Result<Value> {
    let mut data = Map::new();
    insert_text(
        &mut data,
        "text",
        args.text.as_deref(),
        args.text_file.as_deref(),
    )?;
    insert_text(
        &mut data,
        "html_text",
        args.html_text.as_deref(),
        args.html_text_file.as_deref(),
    )?;
    Ok(Value::Object(data))
}

pub fn tag_create(args: &TagCreateArgs, workspace_gid: &str) -> Result<Value> {
    let mut data = Map::new();
    data.insert("name".to_owned(), Value::String(args.name.clone()));
    data.insert(
        "workspace".to_owned(),
        Value::String(workspace_gid.to_owned()),
    );
    insert_option(&mut data, "color", args.color.clone());
    merge_extra(Value::Object(data), args.data.as_deref())
}

pub fn tag_update(args: &TagUpdateArgs) -> Result<Value> {
    let mut data = Map::new();
    insert_option(&mut data, "name", args.name.clone());
    insert_option(&mut data, "color", args.color.clone());
    let value = merge_extra(Value::Object(data), args.data.as_deref())?;
    if value.as_object().is_none_or(Map::is_empty) {
        return Err(Error::Usage(
            "tag update requires --name, --color, or --data".to_owned(),
        ));
    }
    Ok(value)
}

pub fn section_create(args: &SectionCreateArgs) -> Result<Value> {
    let mut data = Map::new();
    data.insert("name".to_owned(), Value::String(args.name.clone()));
    merge_extra(Value::Object(data), args.data.as_deref())
}

fn merge_extra(mut value: Value, source: Option<&str>) -> Result<Value> {
    if let Some(source) = source {
        input::merge_objects(&mut value, input::read_json(source)?)?;
    }
    Ok(value)
}

fn insert_option(data: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        data.insert(key.to_owned(), Value::String(value));
    }
}

fn insert_text(
    data: &mut Map<String, Value>,
    key: &str,
    inline: Option<&str>,
    file: Option<&str>,
) -> Result<()> {
    if let Some(inline) = inline {
        validate_rich_text(key, inline)?;
        data.insert(key.to_owned(), Value::String(inline.to_owned()));
    } else if let Some(file) = file {
        let text = input::read_text(file)?;
        validate_rich_text(key, &text)?;
        data.insert(key.to_owned(), Value::String(text));
    }
    Ok(())
}

fn validate_rich_text(key: &str, value: &str) -> Result<()> {
    if key.starts_with("html_") {
        let value = value.trim();
        if !value.starts_with("<body>") || !value.ends_with("</body>") {
            return Err(Error::Usage(format!(
                "{key} must be valid Asana rich text wrapped in <body>...</body>"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_rejects_overlapping_typed_and_raw_fields() {
        let mut base = serde_json::json!({"name":"typed"});
        assert!(input::merge_objects(&mut base, serde_json::json!({"name":"raw"})).is_err());
    }

    #[test]
    fn task_create_uses_membership_for_section() {
        let args = TaskCreateArgs {
            name: Some("Example".to_owned()),
            assignee: None,
            due_on: None,
            due_at: None,
            start_on: None,
            start_at: None,
            notes: None,
            notes_file: None,
            html_notes: None,
            html_notes_file: None,
            parent: None,
            project: vec!["p1".to_owned()],
            section: Some("s1".to_owned()),
            follower: Vec::new(),
            tag: Vec::new(),
            custom_field: Vec::new(),
            data: None,
        };
        let value = task_create(&args, None, false).expect("valid task");
        assert_eq!(value["memberships"][0]["project"], "p1");
        assert_eq!(value["memberships"][0]["section"], "s1");
        assert!(value.get("projects").is_none());
    }

    #[test]
    fn task_update_clear_due_clears_both_date_representations() {
        let args = TaskUpdateArgs {
            gid: "t1".to_owned(),
            expect_modified_at: None,
            name: None,
            assignee: None,
            clear_assignee: false,
            due_on: None,
            due_at: None,
            clear_due: true,
            start_on: None,
            start_at: None,
            clear_start: false,
            notes: None,
            notes_file: None,
            html_notes: None,
            html_notes_file: None,
            clear_notes: false,
            complete: false,
            incomplete: false,
            set_custom_field: Vec::new(),
            clear_custom_field: Vec::new(),
            data: None,
        };
        let value = task_update(&args).expect("valid patch");
        assert!(value["due_on"].is_null());
        assert!(value["due_at"].is_null());
    }
}
