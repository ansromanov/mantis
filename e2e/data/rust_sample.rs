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

struct Circle {
    radius: f64,
}

impl Circle {
    // A function to calculate area of a circle
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }

    // A function to calculate circumference
    fn circumference(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius
    }
}

struct Square {
    side: u32,
}

impl Square {
    // A function to calculate area of a square
    fn area(&self) -> u32 {
        self.side * self.side
    }

    // Convert to Rectangle
    fn to_rectangle(&self) -> Rectangle {
        Rectangle {
            width: self.side,
            height: self.side,
        }
    }
}

struct Triangle {
    base: f64,
    height: f64,
}

impl Triangle {
    // A function to calculate area of a triangle
    fn area(&self) -> f64 {
        0.5 * self.base * self.height
    }
}

struct Polygon {
    sides: u32,
    side_length: f64,
}

impl Polygon {
    // A function to calculate perimeter
    fn perimeter(&self) -> f64 {
        self.sides as f64 * self.side_length
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

    let circle = Circle { radius: 5.0 };
    println!("circle area: {}, circumference: {}", circle.area(), circle.circumference());

    let square = Square { side: 15 };
    println!("square area: {}", square.area());

    let triangle = Triangle { base: 10.0, height: 8.0 };
    println!("triangle area: {}", triangle.area());

    let pentagon = Polygon { sides: 5, side_length: 6.0 };
    println!("pentagon perimeter: {}", pentagon.perimeter());
}
