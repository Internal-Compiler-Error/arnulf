#![allow(dead_code)]

use std::io::Read;
use futures::{FutureExt, TryFutureExt};
use nom::branch::alt;
use nom::bytes::complete::{tag, take_until1};
use nom::bytes::complete::take_while;
use nom::bytes::complete::take_until;
use nom::character::{complete, is_alphanumeric};
use nom::character::complete::space1;
use nom::character::complete::{digit1, multispace0, newline, space0};
use nom::combinator::{opt, rest};
use nom::IResult;
use nom::multi::many1;
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::Parser;
use crate::{Pragma, TestDirective, TestPoint};

pub fn parse_version(s: &str) -> IResult<&str, &str> {
    tag("TAP Version 14\n")(s)
}

fn parse_test_count(s: &str) -> IResult<&str, &str> {
    preceded(tag("1.."), digit1)(s)
}

fn parse_plan(s: &str) -> IResult<&str, u32> {
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
    use nom::bytes::complete::tag;

    delimited(tag("  ---\n"), rest, tag("  ...\n"))(s)
}

fn parse_comment(s: &str) -> IResult<&str, Option<&str>> {
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
    preceded(multispace0, tag("\n"))(s)
}

fn parse_anything(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::take_until1;

    take_until1("\n")(s)
}

fn parse_pragma(s: &str) -> IResult<&str, Pragma> {
    fn parse_pragma_key(s: &str) -> IResult<&str, &str> {
        take_while(|c| {
            let chr = c as u8;
            is_alphanumeric(chr) || chr == b'-' || chr == b'_'
        })(s)
    }

    let (remaining, pragma) = preceded(tag("pragma "), alt((tag("+"), tag("-"))))(s)?;
    let (remaining, key) = parse_pragma_key(remaining)?;

    let pragma = match pragma {
        "+" => Pragma::Enable(key.to_string()),
        "-" => Pragma::Disable(key.to_string()),
        _ => unreachable!(),
    };

    Ok((remaining, pragma))
}

fn parse_description(s: &str) -> IResult<&str, &str> {
    use nom::bytes::complete::tag;
    use nom::bytes::complete::take_until1;

    let prefix = tag(" -");
    let description = preceded(space1, alt((take_until1("\n"), take_until1(" #"))));

    let (remaining, description) = preceded(opt(prefix), description)(s)?;
    Ok((remaining, description.trim()))
}

fn parse_directive(s: &str) -> IResult<&str, TestDirective> {
    use nom::bytes::complete::tag;
    use nom::bytes::complete::tag_no_case;
    use nom::bytes::complete::take_while;
    use nom::character::complete::space0;
    use nom::bytes::complete::take_until;

    let (remaining, _prefix) = tag(" #")(s)?;
    let (remaining, _prefix) = space0(remaining)?;
    let (remaining, directive) = alt((tag_no_case("todo"), tag_no_case("skip")))(remaining)?;
    let (remaining, _) = take_while(|c: char| { !c.is_whitespace() })(remaining)?;
    let (remaining, reason) = preceded(space0, take_until("\n"))(remaining)?;

    let directive = directive.to_lowercase();
    let reason = reason.trim();
    let reason = if reason.len() == 0 { None } else { Some(reason.to_string()) };
    match &*directive {
        "todo" => Ok((remaining, TestDirective::Todo(reason))),
        "skip" => Ok((remaining, TestDirective::Skip(reason))),
        _ => unreachable!(),
    }
}

fn parse_status(s: &str) -> IResult<&str, bool> {
    let (remaining, status) = alt((tag("ok"), tag("not ok")))(s)?;

    match status {
        "ok" => Ok((remaining, true)),
        "not ok" => Ok((remaining, false)),
        _ => unreachable!(),
    }
}

fn parse_test_number(s: &str) -> IResult<&str, usize> {
    let (remaining, num) = preceded(space1, complete::digit1)(s)?;
    Ok((remaining, num.parse().expect("Test number should be a number")))
}

pub fn parse_test_point(s: &str) -> IResult<&str, Vec<TestPoint>> {
    use nom::character::complete::newline;

    let test_point = tuple((
        parse_status,
        opt(parse_test_number),
        opt(parse_description),
        opt(parse_directive),
        newline,
        opt(parse_yaml),
    )).map(|(status, test_num, description, directive, _newline, yaml)| {
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

    dbg!(many1(test_point)(s))
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use super::*;

    #[test]
    fn test_status() {
        let input = "ok";

        let (_remaining, status) = parse_status(input).unwrap();
        assert_eq!(status, true);
    }

    #[test]
    fn test_test_number() {
        let input = " 1";
        let (_remaining, test_number) = parse_test_number(input).unwrap();
        assert_eq!(test_number, 1);
    }

    #[test]
    fn test_description_no_dash() {
        let input = " this is a description\n";
        let expected = "this is a description";

        let (remaining, parsed) = parse_description(input).unwrap();
        assert_eq!(remaining, "\n");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_description_with_dash() {
        let input = " - this is a description #";
        let expected = "this is a description";

        let (remaining, parsed) = parse_description(input).unwrap();
        assert_eq!(remaining, " #");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_lower_case_directive_with_reason() {
        let input = " #skip this is a directive \n";
        let expected = TestDirective::Skip(Some("this is a directive".to_string()));

        let (remaining, parsed) = parse_directive(input).unwrap();
        assert_eq!(remaining, "\n");
        assert_eq!(parsed, expected)
    }

    #[test]
    fn test_parse_mixed_case_directive_with_reason() {
        let input = " # SKiP another directive   \n";
        let expected = TestDirective::Skip(Some("another directive".to_string()));

        let (remaining, parsed) = parse_directive(input).unwrap();
        assert_eq!(remaining, "\n");
        assert_eq!(parsed, expected)
    }

    #[test]
    fn test_parse_upper_case_directive_with_reason() {
        let input = " #    TODO           is a directive\n";
        let expected = TestDirective::Todo(Some("is a directive".to_string()));

        let (remaining, parsed) = parse_directive(input).unwrap();
        assert_eq!(remaining, "\n");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_upper_case_directive_without_reason() {
        let input = " # TODO\n";
        let expected = TestDirective::Todo(None);

        let (remaining, parsed) = parse_directive(input).unwrap();
        assert_eq!(remaining, "\n");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn test_parse_legacy_directive_with_reason() {
        let input = " #SKIPPED: real reason\n";
        let expected = TestDirective::Skip(Some("real reason".to_string()));

        let (remaining, parsed) = parse_directive(input).unwrap();
        assert_eq!(remaining, "\n");
        assert_eq!(parsed, expected);
    }
}

#[test]
fn test_point() {
    let input = "ok\n";
    let (_remaining, tests) = parse_test_point(input).unwrap();

    let expected = vec![
        TestPoint {
            status: true,
            description: None,
            directive: None,
            yaml: None,
            test_number: None,
        }
    ];
    assert_eq!(tests.len(), 1);
    assert_eq!(tests, expected);
}
