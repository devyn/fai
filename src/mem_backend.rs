pub trait MemBackend {
    type Error;

    fn load(&mut self, addr: u32) -> Result<u32, Self::Error>;

    fn store(&mut self, addr: u32, val: u32) -> Result<(), Self::Error>;
}

impl MemBackend for [u32] {
    type Error = u32;

    fn load(&mut self, addr: u32) -> Result<u32, u32> {
        self.get(addr as usize).cloned().ok_or(addr)
    }

    fn store(&mut self, addr: u32, val: u32) -> Result<(), u32> {
        *(self.get_mut(addr as usize).ok_or(addr)?) = val;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransactionalMemBackend {
    counter: usize,
    committed: Vec<TransactionalMemLog>,
    pending: Option<TransactionalMemRequest>
}

#[derive(Debug, Clone, Copy)]
pub enum TransactionalMemLog {
    Get(u32, u32),
    Set(u32, u32),
}

#[derive(Debug, Clone, Copy)]
pub enum TransactionalMemRequest {
    Get(u32),
    Set(u32, u32)
}

#[derive(Debug, Clone)]
pub enum TransactionalMemError {
    Need(TransactionalMemRequest)
}

impl TransactionalMemBackend {
    pub fn new() -> TransactionalMemBackend {
        TransactionalMemBackend::default()
    }

    /// Faster than creating a new backend. Reuses existing memory allocations.
    pub fn reset(&mut self) {
        self.counter = 0;
        self.committed.truncate(0);
        self.pending = None;
    }

    pub fn retry(&mut self) {
        self.counter = 0;
    }

    pub fn respond_get(&mut self, addr: u32, val: u32) {
        match self.pending {
            Some(TransactionalMemRequest::Get(a)) if addr == a => {
                self.committed.push(TransactionalMemLog::Get(addr, val));
                self.pending = None;
            },
            _ => ()
        }
    }

    pub fn respond_set(&mut self, addr: u32, _val: u32) {
        match self.pending {
            Some(TransactionalMemRequest::Set(a, val)) if addr == a => {
                self.committed.push(TransactionalMemLog::Set(addr, val));
                self.pending = None;
            },
            _ => ()
        }
    }

    pub fn pending(&self) -> Option<TransactionalMemRequest> {
        self.pending
    }
}

impl MemBackend for TransactionalMemBackend {
    type Error = TransactionalMemError;

    fn load(&mut self, addr: u32) -> Result<u32, TransactionalMemError> {
        match self.committed.get(self.counter) {
            Some(&TransactionalMemLog::Get(a, val)) if addr == a => {
                self.counter += 1;
                Ok(val)
            },
            None => {
                let req = TransactionalMemRequest::Get(addr);
                self.pending = Some(req);
                Err(TransactionalMemError::Need(req))
            },
            Some(other) => {
                panic!("non-deterministic condition: load({:#010x}) \
                        called but history records {:?} at this point", addr, other);
            }
        }
    }

    fn store(&mut self, addr: u32, val: u32) -> Result<(), TransactionalMemError> {
        match self.committed.get(self.counter) {
            Some(&TransactionalMemLog::Set(a, v)) if addr == a && val == v => {
                self.counter += 1;
                Ok(())
            },
            None => {
                let req = TransactionalMemRequest::Set(addr, val);
                self.pending = Some(req);
                Err(TransactionalMemError::Need(req))
            },
            Some(other) => {
                panic!("non-deterministic condition: store({:#010x}, {:#010x}) \
                        called but history records {:?} at this point", addr, val, other);
            }
        }
    }
}

// TODO: Write tests for TransactionalMemBackend
