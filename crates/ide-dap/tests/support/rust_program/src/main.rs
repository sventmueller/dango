struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let point = Point { x: 1, y: 2 };
    let mut total = 0;
    for i in 0..3 {
        total += i + point.x + point.y;
    }
    println!("total {total}");
    if std::env::args().any(|arg| arg == "--wait") {
        std::thread::sleep(std::time::Duration::from_secs(120));
    }
}
