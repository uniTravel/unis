use validator::{Validate, ValidationError};

#[derive(Validate)]
struct CreateNote {
    #[validate(length(min = 1, max = 6), custom(function = "validate"))]
    title: String,
    #[validate(length(min = 2))]
    content: String,
}

fn validate(title: &str) -> Result<(), ValidationError> {
    if title == "title" {
        return Err(ValidationError::new("title wrrong"));
    }

    Ok(())
}

fn main() {
    let c = CreateNote {
        title: "title".to_string(),
        content: "content".to_string(),
    };
    match c.validate() {
        Ok(_) => (),
        Err(err) => println!("{err}"),
    };
}
