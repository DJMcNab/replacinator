use std::{fs::File, io::Read};

use replacinator::Replacinator;

#[derive(Debug)]
struct JsonArray<'a> {
    values: Vec<&'a mut str>,
}

fn main() {
    let mut buf = String::new();
    let _ = File::open("examples/test.json")
        .unwrap()
        .read_to_string(&mut buf)
        .unwrap();

    let json_values = Replacinator::new_in(&mut buf, parse_json_array);
    dbg!(json_values);
    println!("Buffer is now: {}", buf);
}

fn parse_json_array<'a>(src: &mut Replacinator<'a>) -> JsonArray<'a> {
    let mut values = Vec::new();
    assert_eq!(src.skip_char(), Some('['));
    loop {
        match src.skip_char() {
            Some('"') => {
                // Reset the replacinator to the beginning of this string
                let _ = src.get_begin();
                loop {
                    match src
                        .read_char()
                        .expect("JSON value should not end in the middle of a string")
                    {
                        '\\' => match src
                            .read_char()
                            .expect("JSON value should not end in the middle of an escape sequence")
                        {
                            '"' => src.write_char('"'),
                            '\\' => src.write_char('\\'),
                            '/' => src.write_char('/'),
                            'b' => src.write_char('\x08'),
                            'f' => src.write_char('\x0c'),
                            'n' => src.write_char('\n'),
                            'r' => src.write_char('\r'),
                            't' => src.write_char('\t'),
                            'u' => {
                                let mut res = 0;
                                for _ in 0..4 {
                                    let v = src
                                        .read_char()
                                        .expect("String ended in unicode")
                                        .to_digit(16)
                                        .expect("Invalid hex digit in escape");
                                    res = res * 16 + v;
                                }
                                src.write_char(
                                    std::char::from_u32(res).expect("Valid character code"),
                                )
                            }
                            other => panic!("Invalid escape {:?}", other),
                        },
                        '"' => {
                            values.push(src.get_begin());
                            src.write_char('"');
                            break;
                        }
                        other => src.write_char(other),
                    }
                }
            }
            Some(']') => break,
            Some(' ') | Some('\n') | Some('\t') => (),
            res => panic!(
                r#"JSON array should be continued with '"' or ']', got {:?}"#,
                res
            ),
        }
    }
    JsonArray { values }
}
