#[cfg(test)]
mod test;

pub(crate) const SID_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
pub(crate) const RID_MASK: u64 = 0x0000_0000_0000_FFFF;

#[inline]
#[must_use]
pub fn generate_response_id(sid: u64, rid: u16) -> u64 {
    (sid << 16) | u64::from(rid)
}

#[inline]
#[must_use]
pub fn next_sid(sid: u64) -> u64 {
    (sid + 1) & SID_MASK
}

#[inline]
#[must_use]
pub fn get_rid(sid: u64) -> u16 {
    // intentional, if masked by 48 it fits into 16
    u16::try_from(sid & RID_MASK).unwrap_or(0)
}
