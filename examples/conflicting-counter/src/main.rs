use edgedb_sdk::{log, web};
use edgedb_sdk::client::{Client, Error, create_client};
use once_cell::sync::Lazy;

static CLIENT: Lazy<Client> = Lazy::new(|| create_client());


fn wrap_error(f: impl FnOnce() -> Result<web::Response, Error>)
    -> web::Response
{
    match f() {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Error handling request: {:#}", e);
            web::response()
                .status(web::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "text/plain")
                .body(format!("Internal Server Error").into())
                .expect("response is built")
        }
    }
}

#[web::handler]
fn handler(req: web::Request) -> web::Response {
    let name = req.uri().path();
    wrap_error(|| {
        let counter = CLIENT.transaction(|tx| {
            let val = tx.query_required_single::<i32, _>("
                SELECT (
                    INSERT Counter {
                        name := <str>$0,
                        value := 1,
                    } UNLESS CONFLICT ON .name
                    ELSE (
                        UPDATE Counter
                        SET { value := .value + 1 }
                    )
                ).value
                ", &(name,),
            )?;
            Ok(val)
        })?;
        Ok(web::response()
            .status(web::StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(format!("Page {name} visited {counter} times").into())
            .expect("response is built"))
    })
}

fn main() {
}
