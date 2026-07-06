// Sample Rust file for Mantis E2E testing
// Tests: Syntax highlighting, folding, line numbers, search

struct Rectangle {
    width: u32,
    height: u32,
}

impl Rectangle {
    // A function to calculate area
    fn area(&self) -> u32 {
        self.width * self.height
    }

    // A function to check if it can hold another
    fn can_hold(&self, other: &Rectangle) -> bool {
        self.width > other.width && self.height > other.height
    }
}

fn main() {
    let rect1 = Rectangle {
        width: 30,
        height: 50,
    };
    let rect2 = Rectangle {
        width: 10,
        height: 40,
    };

    println!("rect1 area: {}", rect1.area());
    println!("Can rect1 hold rect2? {}", rect1.can_hold(&rect2));
}
