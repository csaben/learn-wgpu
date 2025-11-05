// Quick debug script to find Charizard's mouth position
// Run this once, copy the coordinates, then delete this file

fn main() {
    // Load the OBJ file
    let obj_path = std::path::Path::new(env!("OUT_DIR"))
        .join("res/charizard/Charizard.obj");
    let obj_text = std::fs::read_to_string(&obj_path)
        .expect(&format!("Failed to read OBJ at {:?}", obj_path));

    let mut max_y = f32::MIN;
    let mut max_z = f32::MIN;
    let mut max_x = f32::MIN;
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut min_z = f32::MAX;

    // Parse vertices
    for line in obj_text.lines() {
        if line.starts_with("v ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let x: f32 = parts[1].parse().unwrap();
                let y: f32 = parts[2].parse().unwrap();
                let z: f32 = parts[3].parse().unwrap();

                max_x = max_x.max(x);
                min_x = min_x.min(x);
                max_y = max_y.max(y);
                min_y = min_y.min(y);
                max_z = max_z.max(z);
                min_z = min_z.min(z);
            }
        }
    }

    println!("📦 Charizard Bounding Box:");
    println!("  X: [{:.3} to {:.3}]", min_x, max_x);
    println!("  Y: [{:.3} to {:.3}]", min_y, max_y);
    println!("  Z: [{:.3} to {:.3}]", min_z, max_z);
    println!("\n🔥 Suggested fire positions to try:");
    println!("  Head center:  [0.0, {:.3}, {:.3}]", max_y, max_z);
    println!("  Mouth front:  [0.0, {:.3}, {:.3}]", max_y * 0.9, max_z * 1.1);
    println!("  Upper jaw:    [0.0, {:.3}, {:.3}]", max_y * 0.85, max_z);
}
