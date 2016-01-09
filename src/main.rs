extern crate iron;
extern crate router;
extern crate params;
#[macro_use] extern crate hyper;

use iron::prelude::*;
use iron::status;
use iron::mime::Mime;
use router::Router;
use params::{Params,Value};

use std::io::{Read,Write};
use std::fs::File;


header! { (Token, "Token") => [String] }
const AUTH_TOKEN: &'static str = "DEADBEEF";

fn hash_image(image: &[u8]) -> u8 {
    let mut hash = 0u8;
    for x in image {
        hash ^= *x;
    }
    return hash;
}

fn check_auth(token: Option<&Token>) -> Option<Response> {
    match token.map(|&Token(ref s)| &**s) {
        Some(AUTH_TOKEN) => None,
        Some(_) => Some(Response::with((status::Forbidden, "Bad Token\n"))),
        None => Some(Response::with((status::Forbidden, "No Token\n"))),
    }
}

fn image(req: &mut Request) -> IronResult<Response> {
    let mime: Mime = "image/png".parse().unwrap();
    let router = req.extensions.get::<Router>().unwrap();
    let image;
    match router.find("image") {
        Some(query) => image = query,
        _ => return Ok(Response::with((status::BadRequest, "Image not specified\n"))),
    }
    let mut f;
    match File::open(format!("{}", image)) {
        Ok(file) => f = file,
        Err(_) => return Ok(Response::with((status::NotFound, "Image not found\n"))),
    }
    let mut data = Vec::new();
    f.read_to_end(&mut data).unwrap();
    return Ok(Response::with((status::Ok, mime, data)));
}

fn upload(req: &mut Request) -> IronResult<Response> {
    let auth_err = check_auth(req.headers.get::<Token>());
    if auth_err.is_some() { return Ok(auth_err.unwrap()); }

    let mut image = Vec::with_capacity(10);
    let max_len: usize = 8*1024*1024;

    match req.get_ref::<Params>() {
        Ok(params) => {
            match params.get("image") {
                Some(&Value::File(ref upload)) => {
                    if upload.size() > max_len {
                        return Ok(Response::with((status::BadRequest, format!("Image too large, {:?} > {:?}\n", upload.size(), max_len))));
                    } else if upload.size() == 0 {
                        return Ok(Response::with((status::BadRequest,"Empty image data\n"))); 
                    }

                    match upload.open() {
                        Ok(mut handle) => { handle.read_to_end(&mut image).unwrap(); },
                        Err(error) => return Ok(Response::with((status::BadRequest, format!("Failed to read file '{:?}'\n", error)))),
                    }
                },
                _ => return Ok(Response::with((status::BadRequest,"No image data found\n"))),
            }
        },
        Err(error) => return Ok(Response::with((status::BadRequest, format!("Failed to read POST params '{:?}'\n", error)))),
    }

    let hash = hash_image(&image);
    let mut f = File::create(format!("{:02X}", hash)).unwrap();
    f.write_all(&image).unwrap();
    return Ok(Response::with((status::Ok, format!("{:02X}\n", hash))));
}

fn main() {
    let mut router = Router::new();
    router.post("/upload", upload);
    router.get("/:image", image);

    println!("Listening on localhost:8008");
    Iron::new(router).http("localhost:8008").unwrap();

    #[allow(unused_variables, dead_code)]
    fn null(req: &mut Request) -> IronResult<Response> {
        Ok(Response::with((status::Ok, "Hello World\n")))
    }
}


