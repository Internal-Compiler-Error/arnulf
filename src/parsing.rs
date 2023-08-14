#![allow(dead_code)]

use nom::branch::alt;
use nom::bytes::streaming::take_while;
use nom::bytes::streaming::take_until;
use nom::character::{is_alphanumeric};
use nom::character::streaming::{digit1, multispace0, newline, space0};
use nom::combinator::opt;
use nom::IResult;
use nom::multi::many1;
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::Parser;
use crate::{TestDirective, TestPoint};

pub fn parse_version(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    tag("TAP Version 14\n")(s)
}

fn parse_test_count(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    preceded(tag("1.."), digit1)(s)
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

    delimited(tag("  ---\n"), take_while(|c: char| { c.is_ascii() }), tag("  ...\n"))(s)
}

fn parse_comment(s: &str) -> IResult<&str, Option<&str>> {
    use nom::bytes::streaming::tag;

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

        take_while(|c| {
            let chr = c as u8;
            is_alphanumeric(chr) || chr == b'-' || chr == b'_'
        })(s)
    }

    let (remaining, _pragma) = preceded(tag("pragma "), alt((tag("+"), tag("-"))))(s)?;

    // TODO: figure out a good way to represent + pragma and - pragma
    parse_pragma_key(remaining)
}


fn parse_description(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;
    use nom::bytes::streaming::take_till1;

    let prefix = tag(" -");
    let description = take_till1(|c| {
        c == '#'
    });

    preceded(opt(prefix), preceded(tag(" "), description))(s)
}

fn parse_directive(s: &str) -> IResult<&str, TestDirective> {
    use nom::bytes::streaming::tag_no_case;
    use nom::bytes::streaming::tag;

    let (remaining, directive) = preceded(tag("# "), alt((tag_no_case("todo"), tag_no_case("skip"))))(s)?;
    let (remaining, reason) = opt(preceded(tag(" "), take_until("\n")))(remaining)?;

    match directive {
        "todo" => {
            match reason {
                Some(reason) => Ok((remaining, TestDirective::Todo(Some(reason.to_string())))),
                None => Ok((remaining, TestDirective::Todo(None))),
            }
        }
        "skip" => {
            match reason {
                Some(reason) => Ok((remaining, TestDirective::Skip(Some(reason.to_string())))),
                None => Ok((remaining, TestDirective::Skip(None))),
            }
        }
        _ => unreachable!(),
    }
}

pub fn parse_test_point(s: &str) -> IResult<&str, Vec<TestPoint>> {
    use nom::bytes::streaming::tag;

    let parse_test_number = preceded(tag(" "), digit1);
    let status = alt((tag("ok"), tag("not ok")));

    let test_point = tuple((
        status,
        opt(parse_test_number),
        opt(parse_description),
        opt(parse_directive),
        newline,
        opt(parse_yaml),
    )).map(|(status, test_num, description, directive, _newline, yaml)| {
        let status = match status {
            "ok" => true,
            "not ok" => false,
            _ => unreachable!(),
        };

        let test_num = test_num.map(|test_num| {
            test_num.parse::<usize>().expect("Test number should be a number")
        });

        let description = description.map(|description| {
            description.to_string()
        });

        let yaml = yaml.map(|yaml| {
            yaml.to_string()
        });

        TestPoint {
            status,
            description,
            directive,
            yaml,
            test_number: test_num,
        }
    });

    many1(test_point)(s)
}
