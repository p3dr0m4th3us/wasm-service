use crate::Runnable;
use crate::{
    logging::{self, AppendsLog},
    Method, Request, Response,
};
use std::panic::UnwindSafe;
use url::Url;

/// Context manages the information flow for an incoming http Request,
/// the application handler, and the generated http Response. It holds a buffer
/// for log messages, and a hook for deferred tasks to be processed after the Response is returned.
pub struct Context {
    request: Request,
    response: Response,
    log_queue: logging::LogQueue,
    deferred: Vec<Box<dyn Runnable + UnwindSafe>>,
    default_content_type: Option<String>,
}

impl Context {
    /// Constructs a new Context with the received Request.
    pub(crate) fn new(request: Request) -> Self {
        Self {
            request,
            response: Response::default(),
            log_queue: logging::LogQueue::default(),
            deferred: Vec::new(),
            default_content_type: None,
        }
    }

    /// Accesses the Request object
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Returns the request http method
    pub fn method(&self) -> Method {
        self.request.method()
    }

    /// Returns the parsed Url of the incoming request
    pub fn url(&self) -> &Url {
        &self.request.url()
    }

    /// Returns the request body, or None if the body was empty
    pub fn body(&self) -> Option<&Vec<u8>> {
        self.request.body()
    }

    /// Creates response builder
    pub fn response(&mut self) -> &mut Response {
        &mut self.response
    }

    /// Adds a task to the deferred task queue. The task queue uses
    /// [event.waitUntil](https://www.w3.org/TR/service-workers/#extendableevent)
    /// to extend the lifetime of the request event, and are run after the response
    /// has been returned to the client. Deferred tasks are good for logging
    /// and analytics.
    pub fn defer(&mut self, task: Box<dyn Runnable + UnwindSafe>) {
        self.deferred.push(task);
    }

    /// Sets default content type for the Response.
    /// If not overridden later by `header("Content-Type", ...)`
    /// this value will be used. It may be useful to set this at the top of the handler/router,
    /// to handle the most common response media type, and override it for special cases.
    pub fn default_content_type<T: Into<String>>(&mut self, ct: T) {
        self.default_content_type = Some(ct.into())
    }

    /// Returns pending log messages, emptying internal queue.
    /// This is used for sending queued messages to an external log service
    pub(crate) fn take_logs(&mut self) -> Vec<logging::LogEntry> {
        self.log_queue.take()
    }

    /// Returns deferred tasks, emptying internal list
    pub(crate) fn take_tasks(&mut self) -> Vec<Box<dyn Runnable + UnwindSafe>> {
        std::mem::take(&mut self.deferred)
    }

    /// Returns response, replacing self.response with default
    pub(crate) fn take_response(&mut self) -> Response {
        std::mem::take(&mut self.response)
    }
}

impl AppendsLog for Context {
    /// Adds log to deferred queue
    fn log(&mut self, e: logging::LogEntry) {
        self.log_queue.log(e);
    }
}

mod test {
    use crate::{Context, Method, Request};
    use url::Url;
    use wasm_bindgen_test::wasm_bindgen_test;

    // internal helper function to create a dummy Request
    fn make_req(url: &'static str) -> Request {
        Request::new(
            Method::GET,
            Url::parse(url).expect("url"),
            web_sys::Headers::new().unwrap(),
            None,
        )
    }

    #[wasm_bindgen_test]
    fn response_defaults() {
        let req = make_req("https://www.example.com");
        let mut ctx = crate::Context::new(req);
        assert_eq!(ctx.response().get_status(), 200);
        assert_eq!(ctx.response().get_body().len(), 0);
    }

    #[wasm_bindgen_test]
    fn response_text() {
        let req = make_req("https://www.example.com");
        let mut ctx = Context::new(req);
        ctx.response().status(201).text("hello");

        assert_eq!(ctx.response().get_status(), 201);
        assert_eq!(&ctx.response().get_body(), &b"hello");
    }

    #[wasm_bindgen_test]
    fn response_bin() {
        let req = make_req("https://www.example.com");
        let mut ctx = Context::new(req);
        ctx.response().status(202).body(b"bytes");

        assert_eq!(ctx.response().get_status(), 202);
        assert_eq!(&ctx.response().get_body(), &b"bytes");
    }

    #[wasm_bindgen_test]
    fn response_headers() {
        let req = make_req("https://www.example.com");
        let mut ctx = Context::new(req);
        ctx.response()
            .header("Content-Type", "application/json")
            .expect("set-header");

        let sv = ctx
            .response
            .get_headers()
            .unwrap()
            .get("Content-Type")
            .expect("get header");
        assert!(sv.is_some(), "is-defined content-type");
        assert_eq!(sv.unwrap(), "application/json", "content-type value");
    }
}
