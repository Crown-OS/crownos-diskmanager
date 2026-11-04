const MIN_FAIR_LENGTH: usize = 8;
const MIN_STRONG_LENGTH: usize = 12;
const PASSPHRASE_LENGTH: usize = 16;
const STRONG_VARIETY: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PasswordStrength {
    Weak,
    Fair,
    Strong,
}

impl PasswordStrength {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Weak => "Weak password",
            Self::Fair => "Fair password",
            Self::Strong => "Strong password",
        }
    }
}

fn variety(password: &str) -> usize {
    let classes: [fn(char) -> bool; 4] = [
        char::is_lowercase,
        char::is_uppercase,
        |character| character.is_ascii_digit(),
        |character| !character.is_alphanumeric(),
    ];
    classes
        .iter()
        .filter(|class| password.chars().any(**class))
        .count()
}

pub fn password_strength(password: &str) -> PasswordStrength {
    let length = password.chars().count();
    match (length, variety(password)) {
        (length, _) if length < MIN_FAIR_LENGTH => PasswordStrength::Weak,
        (length, _) if length >= PASSPHRASE_LENGTH => PasswordStrength::Strong,
        (length, variety) if length >= MIN_STRONG_LENGTH && variety >= STRONG_VARIETY => {
            PasswordStrength::Strong
        }
        (_, 1) => PasswordStrength::Weak,
        _ => PasswordStrength::Fair,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_passwords_are_weak() {
        assert_eq!(password_strength(""), PasswordStrength::Weak);
        assert_eq!(password_strength("Ab1!x"), PasswordStrength::Weak);
    }

    #[test]
    fn one_kind_of_character_stays_weak_until_it_is_long() {
        assert_eq!(password_strength("abcdefghij"), PasswordStrength::Weak);
        assert_eq!(
            password_strength("correcthorsebattery"),
            PasswordStrength::Strong
        );
    }

    #[test]
    fn variety_and_length_make_it_stronger() {
        assert_eq!(password_strength("sunny1day"), PasswordStrength::Fair);
        assert_eq!(password_strength("Sunny-Day-42"), PasswordStrength::Strong);
    }
}
