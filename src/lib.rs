#![allow(dead_code)]

use futures::{AsyncBufReadExt, Stream, AsyncReadExt};
use std::{io::Result, pin::Pin, task::{Context, Poll}, io};
use std::future::Future;
use pin_project::pin_project;
use crate::parsing::{parse_test_point, parse_version};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum TestDetails {
    TestPoint(TestPoint),
    BailOut(BailOut),
    TestPlan(TestPlan),
    Pragma(Pragma),
    Comment(String),
    Empty,
    Anything(String),
    // todo: implement subtest
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Comment(pub String);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct TestPlan(pub usize);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct BailOut(pub String);


#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Pragma {
    Enable(String),
    Disable(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct TestPoint {
    pub status: bool,
    pub test_number: Option<usize>,
    pub description: Option<String>,
    pub directive: Option<TestDirective>,
    pub yaml: Option<String>,
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum TestDirective {
    Todo(Option<String>), Skip(Option<String>)
}
mod parsing;

struct Parser<T> {
    stream: T,
}


#[pin_project]
struct ResultStream<T>
{
    #[pin]
    stream: T,
    buffer: Vec<u8>,
}

impl<T> ResultStream<T>
    where T: AsyncReadExt
{
    /// Polls the underlying stream for read readiness.
    fn poll_for_read(stream: &mut Pin<&mut T>, buffer: &mut Vec<u8>, cx: &mut Context<'_>) -> Poll<io::Result<usize>> {
        let mut fut = stream.read_to_end(buffer);
        let fut = Pin::new(&mut fut);
        fut.poll(cx)
    }
}

impl<T> Stream for ResultStream<T>
    where
        T: AsyncReadExt
{
    type Item = io::Result<TestPoint>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // read from the stream
        let read = Self::poll_for_read(&mut this.stream, this.buffer, cx);

        // if the stream is not ready, return
        match read {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(e)) => return Poll::Ready(Some(Err(e))),
            Poll::Ready(Ok(0)) => todo!(),
            Poll::Ready(Ok(_)) => {}
        }

        // parse the buffer
        let string = std::str::from_utf8(this.buffer).expect("buffer should be utf8");
        let test_point = parse_test_point(string);

        match test_point {
            // we parsed a test point, return it, but keep the rest of the buffer
            #[allow(unused_variables)]
            Ok((remaining, test_point)) => {
                // discard the parsed part of the buffer
                this.buffer.drain(0..(string.len() - remaining.len()));
                todo!()
            }
            // if we can need more data, keep reading
            Err(nom::Err::Incomplete(_)) => {
                return Poll::Pending;
            }
            Err(_) => todo!(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<T> Parser<T>
    where
        T: AsyncBufReadExt + Unpin,
{
    pub async fn new(mut stream: T) -> Result<Parser<T>> {
        // We only parse tap version 14
        let mut buffer = String::new();
        stream.read_line(&mut buffer).await?;
        let (_remaining, _version) = parse_version(&*buffer).unwrap();

        Ok(Parser {
            stream,
        })
    }

    pub fn test_results(self) -> ResultStream<T> {
        ResultStream {
            stream: self.stream,
            buffer: Vec::new(),
        }
    }
}
