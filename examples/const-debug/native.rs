struct Provider;

impl generated::Native for Provider {
    fn print_i64(value: i64) {
        println!("{value}");
    }
}
