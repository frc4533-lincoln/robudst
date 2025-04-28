use crate::Ds;

pub(crate) mod tcp;
pub(crate) mod udp;

pub(crate) trait IncomingTagHandler<'d> {
    fn handle(&self, ds: &'d Ds);
}
