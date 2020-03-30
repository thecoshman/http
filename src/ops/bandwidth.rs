use iron::{AfterMiddleware, IronResult, Response, Handler, Request};
use std::num::{NonZeroUsize, NonZeroU64};
use std::io::{Result as IoResult, Write};
use iron::response::WriteBody;
use std::time::Duration;
use std::thread;


pub const DEFAULT_SLEEP: Duration = Duration::from_millis(1);


#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SimpleChain<H: Handler, Am: AfterMiddleware> {
    pub handler: H,
    pub after: Option<Am>,
}

impl<H: Handler, Am: AfterMiddleware> Handler for SimpleChain<H, Am> {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let resp = self.handler.handle(req)?;
        match self.after.as_ref() {
            Some(am) => am.after(req, resp),
            None => Ok(resp),
        }
    }
}



#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct LimitBandwidthMiddleware {
    pub bandwidth: NonZeroU64,
}

impl LimitBandwidthMiddleware {
    pub fn new(bandwidth: NonZeroU64) -> LimitBandwidthMiddleware {
        LimitBandwidthMiddleware { bandwidth: bandwidth }
    }
}

impl AfterMiddleware for LimitBandwidthMiddleware {
    fn after(&self, _: &mut Request, res: Response) -> IronResult<Response> {
        Ok(Response {
            body: res.body.map(|body| {
                Box::new(LimitBandwidthWriteBody {
                    bandwidth: self.bandwidth,
                    underlying: body,
                }) as Box<dyn WriteBody>
            }),
            ..res
        })
    }
}


struct LimitBandwidthWriteBody {
    bandwidth: NonZeroU64,
    underlying: Box<dyn WriteBody>,
}

impl WriteBody for LimitBandwidthWriteBody {
    fn write_body(&mut self, res: &mut dyn Write) -> IoResult<()> {
        self.underlying.write_body(&mut LimitBandwidthWriter::new(self.bandwidth, res))
    }
}


struct LimitBandwidthWriter<'o> {
    chunk_len: NonZeroUsize,
    output: &'o mut dyn Write,
}

impl<'o> LimitBandwidthWriter<'o> {
    fn new(bandwidth: NonZeroU64, output: &'o mut dyn Write) -> LimitBandwidthWriter<'o> {
        LimitBandwidthWriter {
            // bandwidth / (1000 / DEFAULT_SLEEP_MS)
            chunk_len: NonZeroUsize::new(bandwidth.get() as usize * DEFAULT_SLEEP.as_millis() as usize / 1000).unwrap_or(NonZeroUsize::new(1).unwrap()),
            output: output,
        }
    }
}

impl<'o> Write for LimitBandwidthWriter<'o> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        self.output.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> IoResult<()> {
        for chunk in buf.chunks(self.chunk_len.get()) {
            self.output.write_all(chunk)?;
            self.output.flush()?;
            thread::sleep(DEFAULT_SLEEP);
        }

        Ok(())
    }
}
