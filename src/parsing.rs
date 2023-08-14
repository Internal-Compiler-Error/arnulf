#![allow(dead_code)]
use nom::branch::alt;
use nom::character::streaming::{multispace0, space0};
use nom::IResult;
use nom::sequence::{preceded, terminated};

pub fn parse_version(s: &str) -> IResult<&str, &str> {
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

pub fn parse_test_point(s: &str) -> IResult<&str, &str> {
    use nom::bytes::streaming::tag;

    let (remaining, status) = alt((tag("ok"), tag("not ok")))(s)?;
    let (remaining, test_number) = preceded(tag(" "), parse_number)(remaining)?;

    todo!()
}
