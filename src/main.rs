use std::{env, error::Error};
use reqwest::Client;
use git2::Repository;
use serde_json::{json, Value};
use dotenv::dotenv;


fn get_diff(repo: &Repository) -> Result<String, git2::Error>{
    let mut index = repo.index()?;
    let diff = repo.diff_index_to_workdir(Some(&mut index), None)?;

    let mut diff_content = String::new();

    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let content_str = std::str::from_utf8(line.content()).unwrap_or_default();
        let line_str = match line.origin() {
            ' ' => format!(" {}", content_str),
            '-' => format!("-{}", content_str),
            '+' => format!("+{}", content_str),
            _ => String::new(),
        };
        diff_content.push_str(&line_str);
        true
    })?;

    Ok(diff_content)
}

async fn send_openai_request(api_key: &str, prompt: &str) -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let params = json!({
        "model": "gpt-3.5-turbo",
        "messages": [
            {
                "role": "user",
                "content": prompt
            }
        ]
    });

    let res = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&params)
        .send()
        .await?
        .json()
        .await?;

    log_result(&res);

    Ok(())
}

fn log_result(res: &Value) {
    if let Some(choices) = res.get("choices") {
        if let Some(choice) = choices.as_array().and_then(|arr| arr.get(0)) {
            if let Some(message) = choice.get("message") {
                if let Some(content) = message.get("content") {
                    println!("Response: {}", content);
                }
            }
        }
    }
}


#[tokio::main]
async fn main() -> Result<(), git2::Error> {
    dotenv().ok();
    let openai_api_key =  env::var("OPENAI_API_KEY").unwrap();
    let current_dir = env::current_dir().map_err(|e| {
        git2::Error::from_str(&format!("Failed to get current directory: {}", e))
    })?;
    println!("Current directory: {:?}", current_dir);

    let repo = Repository::open(&current_dir).map_err(|e| {
        git2::Error::from_str(&format!("Failed to open repository: {}", e))
    })?;
    
    let change_diff = get_diff(&repo)?;

    let prompt = format!("Generate a semmantic commit basaed on the following change diff, commit should not be more than 100chars and prefixs are [feat:, chore:, refactor:, fix: ], do not mention change diff in you commit \n change diff: {}", change_diff); 

    let commit  = send_openai_request(&openai_api_key, &prompt).await;

    println!("{:?}", commit);
    Ok(())
}
