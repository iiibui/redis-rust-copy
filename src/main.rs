use redis_rust_copy::Sds;

fn main() {
    let mut s = Sds::from_str("Hi, ");
    s.push_str("redis ").push_str("rust.");

    assert_eq!("Hi, redis rust.".len(), s.len());
    println!("{}", s);
}