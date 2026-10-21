use chrono::{DateTime, Local};

pub struct Clock(Box<dyn Fn() -> DateTime<Local>>);

impl Clock {
    pub fn system() -> Self {
        Self(Box::new(Local::now))
    }

    #[cfg(test)]
    pub fn fixed(moment: DateTime<Local>) -> Self {
        Self(Box::new(move || moment))
    }

    pub fn now(&self) -> DateTime<Local> {
        (self.0)()
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::system()
    }
}

#[cfg(test)]
mod tests;
