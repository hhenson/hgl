struct Provider;

impl generated::Native for Provider {
    fn as_str_i64(value: i64) -> String {
        value.to_string()
    }

    fn print_line_str(text: String) {
        println!("{text}");
    }
}
