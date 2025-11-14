use sp_core::U256;

pub const MASK: [u64; 64] = [
    0x1,
    0x2,
    0x4,
    0x8,
    0x10,
    0x20,
    0x40,
    0x80,
    0x100,
    0x200,
    0x400,
    0x800,
    0x1000,
    0x2000,
    0x4000,
    0x8000,
    0x10000,
    0x20000,
    0x40000,
    0x80000,
    0x100000,
    0x200000,
    0x400000,
    0x800000,
    0x1000000,
    0x2000000,
    0x4000000,
    0x8000000,
    0x10000000,
    0x20000000,
    0x40000000,
    0x80000000,
    0x100000000,
    0x200000000,
    0x400000000,
    0x800000000,
    0x1000000000,
    0x2000000000,
    0x4000000000,
    0x8000000000,
    0x10000000000,
    0x20000000000,
    0x40000000000,
    0x80000000000,
    0x100000000000,
    0x200000000000,
    0x400000000000,
    0x800000000000,
    0x1000000000000,
    0x2000000000000,
    0x4000000000000,
    0x8000000000000,
    0x10000000000000,
    0x20000000000000,
    0x40000000000000,
    0x80000000000000,
    0x100000000000000,
    0x200000000000000,
    0x400000000000000,
    0x800000000000000,
    0x1000000000000000,
    0x2000000000000000,
    0x4000000000000000,
    0x8000000000000000,
];


pub fn xor(a: U256, b: U256, start_a: usize, start_b: usize) -> U256{
    match start_a.cmp(&start_b) {
        std::cmp::Ordering::Equal => b ^ a,
        std::cmp::Ordering::Less => {
            let diff = start_b - start_a;
            ((b >> diff) ^ a) << diff
        }
        std::cmp::Ordering::Greater => {
            let diff = start_b - start_a;
            ((b << diff) ^ a) << diff
        }
    }
}

/// Sort by arr[i].1
pub fn radix_sort(arr: &mut Vec<(usize, usize)>, max: usize) {
    let mut exp = 1;
    loop {
        if max / exp == 0 {
            break;
        }
        *arr = count_sort(arr, exp);
        exp *= 10;
    }
}

fn count_sort(arr: &Vec<(usize, usize)>, exp: usize) -> Vec<(usize, usize)> {
    let mut count = [0usize; 10];

    arr.iter().for_each(|(_, b)| count[(b / exp) % 10] += 1);

    for i in 1..10 {
        count[i] += count[i - 1];
    }

    let mut output = vec![(0usize, 0usize); arr.len()];

    arr.iter().rev().for_each(|(a, b)| {
        output[count[(b / exp) % 10] - 1] = (*a, *b);
        count[(b / exp) % 10] -= 1;
    });

    output
}
