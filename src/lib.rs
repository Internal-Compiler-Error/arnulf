#![allow(dead_code)]

use futures::{AsyncBufReadExt, Stream, StreamExt, AsyncReadExt};
use nom::{
    character::streaming::{multispace0, space0},
    sequence::{preceded, terminated},
    IResult,
    branch::alt,
};
use std::{io::Result, pin::Pin, task::{Context, Poll}, io};
use std::future::Future;
use pin_project::pin_project;

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

pub struct Comment(pub String);

pub struct TestPlan(pub usize);

pub struct BailOut(pub String);

pub enum Pragma {
    Enable(String),
    Disable(String),
}

pub struct TestPoint {
    pub status: bool,
    pub test_number: Option<u32>,
    pub description: Option<String>,
    pub yaml: Option<String>,
}

struct Parser<T> {
    stream: T,
}

fn parse_version(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    tag("TAP Version 14\n")(s)
}

fn parse_number(s: &str) -> IResult<&str, &str> {
    use nom::character::streaming::digit1;

    digit1(s)
}

fn parse_test_count(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    preceded(tag("1.."), parse_number)(s)
}

fn parse_plan(s: &str) -> IResult<&str, u32> {
    use nom::bytes::streaming::tag;

    fn parse_reason(s: &str) -> IResult<&str, &str> {
        use nom::bytes::streaming::take_until1;

        preceded(tag(" # "), take_until1("\n"))(s)
    }

    fn parse_remaining(s: &str) -> IResult<&str, &str> {
        alt((parse_reason, tag("\n")))(s)
    }

    let (remaining, count) = parse_test_count(s)?;

    let count: u32 = count
        .parse()
        .expect("Parsing should guaranteed test count to be a number");
    Ok((remaining, count))
}

fn parse_bail_out(s: &str) -> IResult<&str, Option<&str>> {
    use nom::bytes::streaming::tag;
    use nom::bytes::streaming::take_until;

    fn parse(s: &str) -> IResult<&str, &str> {
        preceded(tag("Bail out!"), take_until("\n"))(s)
    }
    let (remaining, reason) = terminated(parse, tag("\n"))(s)?;

    if reason.is_empty() {
        Ok((remaining, None))
    } else {
        Ok((remaining, Some(reason)))
    }
}

fn parse_yaml(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;
    use nom::bytes::streaming::take_until;

    fn parse(s: &str) -> IResult<&str, &str> {
        preceded(tag("  ---\n"), take_until("  ...\n"))(s)
    }

    parse(s)
}

fn parse_comment(s: &str) -> IResult<&str, Option<&str>> {
    use nom::bytes::streaming::tag;
    use nom::bytes::streaming::take_until;

    fn parse(s: &str) -> IResult<&str, &str> {
        preceded(tag("#"), take_until("\n"))(s)
    }

    let (remaining, comment) = preceded(space0, parse)(s)?;

    if comment.is_empty() {
        Ok((remaining, None))
    } else {
        Ok((remaining, Some(comment)))
    }
}

fn parse_empty(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    preceded(multispace0, tag("\n"))(s)
}

fn parse_anything(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::take_until1;

    take_until1("\n")(s)
}

fn parse_pragma(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    fn parse_pragma_key(s: &str) -> IResult<&str, &str> {
        use nom::bytes::streaming::take_while;
        use nom::character::is_alphanumeric;

        take_while(|c| {
            let chr = c as u8;
            is_alphanumeric(chr) || chr == b'-' || chr == b'_'
        })(s)
    }

    let (remaining, pragma) = preceded(tag("pragma "), alt((tag("+"), tag("-"))))(s)?;

    // TODO: figure out a good way to represent + pragma and - pragma
    parse_pragma_key(remaining)
}


fn parse_description(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;
    use nom::bytes::streaming::take_till1;

    let (remaining, description) = alt((tag(" - "), tag(" ")))(s)?;
    take_till1(|c| {
        let chr = c as u8;
        chr == b'#' || chr == b'\n'
    })(s)
}

fn test_directive(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag_no_case;
    use nom::bytes::streaming::tag;

    let (remaining, reason) = preceded(tag("#"), alt((tag_no_case("todo"), tag_no_case("skip"))))(s)?;
    // todo: make a better type
    preceded(tag(" "), parse_anything)(reason)
}

fn parse_test_point(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    let (remaining, status) = alt((tag("ok"), tag("not ok")))(s)?;
    let (remaining, test_number) = preceded(tag(" "), parse_number)(remaining)?;

    todo!()
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
        let (remaining, _version) = parse_version(&*buffer).unwrap();

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
