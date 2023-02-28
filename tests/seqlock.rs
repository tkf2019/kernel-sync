use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;

use kernel_sync::SeqLock;

#[test]
fn test() {
    const N: usize = 20;

    let data = Arc::new(SeqLock::new(0));

    let (tx, rx) = channel();
    for _ in 0..N {
        let (data, tx) = (Arc::clone(&data), tx.clone());
        thread::spawn(move || {
            let mut lock = data.write();
            *lock += 1;
            drop(lock);

            let mut read = 0;
            if data.try_read(|data| read = *data) {
                println!("{:?} read successfully: {}", thread::current().id(), read);
            }

            if read == N {
                tx.send(()).unwrap();
            }
        });
    }

    rx.recv().unwrap();
}
