use std::{env, error::Error};
use reqwest::Client;
use git2::Repository;
use serde_json::{json, Value};
use dotenv::dotenv;


fn commit_new_changes(repo: &Repository, message: &str) -> Result<(), git2::Error> {
    let mut index = repo.index()?;
    let oid = index.write_tree()?;
    let tree = repo.find_tree(oid)?;

    let head = repo.head()?.peel_to_commit()?;
    let parent_commit = head.id();

    let signature = repo.signature()?;
    let commit_id = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&head],
    )?;

    let commit = repo.find_commit(commit_id)?;
    println!("New commit: {}", commit.id());

    Ok(())
}


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

async fn send_openai_request(api_key: &str, change_diff: String) -> Result<String, Box<dyn Error>> {
    let prompt = format!("Generate a semantic commit based on the following change diff, commit should not be more than 100 chars and prefixes are [feat:, chore:, refactor:, fix: ], do not mention change diff in your commit \n change diff: {}", change_diff);

    let client = Client::new();
    let params = json!({
        "model": "gpt-4o",
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

    let commit_message = log_result(&res);
    Ok(commit_message) 
}


fn log_result(res: &Value) -> String {
    if let Some(choices) = res.get("choices") {
        if let Some(choice) = choices.as_array().and_then(|arr| arr.get(0)) {
            if let Some(message) = choice.get("message") {
                if let Some(content) = message.get("content") {
                    let commit_message = content.to_string().trim_matches('"').to_string();
                    println!("Generated commit message: {}", commit_message);
                    return commit_message;
                }
            }
        }
    }
    String::new()
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

   
    let commit_message = match send_openai_request(&openai_api_key, change_diff).await {
        Ok(message) => message,
        Err(err) =>  panic!("Something went wrong {:?}", err),
    };

    let resu = commit_new_changes(&repo, &commit_message);

    println!("{:?}", resu);
    Ok(())
}
