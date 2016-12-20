extern crate iron;
extern crate router;
extern crate params;
#[macro_use] extern crate hyper;
extern crate serde_json;
extern crate liquid;

use iron::prelude::*;
use iron::status;
use iron::modifiers::Header;
use hyper::header::{ETag, EntityTag};
use iron::mime::Mime;
use router::Router;
use params::{Params,Value};
use liquid::{Renderable, Context};

use std::io::{Read,Write};
use std::fs::File;
use std::fs;


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
    let image = match router.find("image") {
        Some(query) => query,
        _ => return Ok(Response::with((status::BadRequest, "Image not specified\n"))),
    };

    let timestamp = match fs::metadata(format!("images/{}", image)) {
        Ok(meta) => format!("{:X}", meta.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),
        Err(e) => return Ok(Response::with((status::InternalServerError, format!("Error getting image timestamp: {:#?}\n", e)))),
    };
    let etag_header = Header(ETag(EntityTag::new(true, timestamp.clone())));
    match router.find("If-None-Match") {
        Some(p) if p == timestamp => return Ok(Response::with((status::NotModified, etag_header))),
        _ => {},
    };
    
    let mut f = match File::open(format!("images/{}", image)) {
        Ok(file) => file,
        _ => return Ok(Response::with((status::NotFound, "Image not found\n"))),
    };
    let mut data = Vec::new();
    f.read_to_end(&mut data).unwrap();
    return Ok(Response::with((status::Ok, mime, etag_header, data)));
}

fn upload(req: &mut Request) -> IronResult<Response> {
    let auth_err = check_auth(req.headers.get::<Token>());
    if auth_err.is_some() { return Ok(auth_err.unwrap()); }

    let mut image = Vec::with_capacity(10);
    let max_len: u64 = 8*1024*1024;

    match req.get_ref::<Params>() {
        Ok(params) => {
            match params.get("image") {
                Some(&Value::File(ref upload)) => {
                    if upload.size > max_len {
                        return Ok(Response::with((status::BadRequest, format!("Image too large, {:?} > {:?}\n", upload.size, max_len))));
                    } else if upload.size == 0 {
                        return Ok(Response::with((status::BadRequest,"Empty image data\n"))); 
                    }

                    match upload.open() {
                        Ok(mut handle) => { handle.read_to_end(&mut image).unwrap(); },
                        Err(error) => return Ok(Response::with((status::BadRequest, format!("Failed to read file '{:#?}'\n", error)))),
                    }
                },
                _ => return Ok(Response::with((status::BadRequest,"No image data found\n"))),
            }
        },
        Err(error) => return Ok(Response::with((status::BadRequest, format!("Failed to read POST params '{:#?}'\n", error)))),
    }

    let hash = hash_image(&image);
    let mut f = File::create(format!("images/{:02X}", hash)).unwrap();
    f.write_all(&image).unwrap();
    return Ok(Response::with((status::Ok, format!("{:02X}\n", hash))));
}

fn get_all(_: &mut Request) -> IronResult<Response> {
    let mut file =  match fs::File::open("./image_index.html") {
        Ok(f) => f,
        _ => return Ok(Response::with((status::InternalServerError, "Failed to read index"))),
    };

    let mut string = String::new();
    file.read_to_string(&mut string).unwrap();

    let template = match liquid::parse(&string, Default::default()) {
        Ok(t) => t,
        Err(e) => return Ok(Response::with((status::InternalServerError, format!("Failed to parse liquid template with error: {:#?}\n", e)))),
    };

    let mut context = Context::new();
    let mut images = Vec::new();

    let image_dir = match fs::read_dir("./images") {
        Ok(dir) => dir,
        Err(err) => return Ok(Response::with((status::InternalServerError, format!("Failed to read image dir: {:#?}\n", err)))),
    };

    for file_iter in image_dir {
        let file = file_iter.unwrap();
        let meta = fs::metadata(file.path()).unwrap();
        if meta.is_file() {
            images.push(file.file_name().into_string().unwrap());
        }
    }
    context.set_val("images", liquid::Value::Array(images.into_iter().map(|e| {liquid::Value::Str(e)} ).collect()));
    let templated = match template.render(&mut context) {
        Ok(Some(t)) => t,
        _ => return Ok(Response::with((status::InternalServerError, "Failed to render liquid template"))),
    };

    let mime: Mime = "text/html".parse().unwrap();
    return Ok(Response::with((status::Ok, mime, templated)))
}

fn main() {
    let mut router = Router::new();
    router.post("/upload", upload, "upload");
    router.get("/all", get_all, "get_all");
    router.get("/:image", image, "get_image");

    println!("Listening on localhost:8008");
    Iron::new(router).http("localhost:8008").unwrap();

    #[allow(unused_variables, dead_code)]
    fn null(req: &mut Request) -> IronResult<Response> {
        Ok(Response::with((status::Ok, "Hello World\n")))
    }
}
