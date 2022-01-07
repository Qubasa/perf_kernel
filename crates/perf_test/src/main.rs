


fn main() {
    println!("Hello, world!");
    let mut dst: [f32; 10] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
    let src: [f32; 10] = [0.1, 0.2, 0.31, 0.42, 0.52, 0.62, 0.72, 0.82, 0.9, 1.0];
    perf_test::mix_mono_to_stereo(&mut dst, &src, 0.2, 0.3);
    println!("dst: {:?}", dst);
}
