Ha, I didn't expect this to show up on HN!

Release is out now, including a blog post at [https://www.mitmproxy.org/posts/releases/mitmproxy-12.3/](https://www.mitmproxy.org/posts/releases/mitmproxy-12.3/). There isn't really much to explain, you just pass \`--emudhra\`. :)
=====
I created a proxy using the 6 compromised eMudhra CA keys (2 of the 8 are code-signing and not for TLS). I initially tried just one (the one with d=4) like in the commit, but it did not seem to work for anything. Once I added all 6 keys, I was able to intercept some.

I was mainly interested in testing which IoT devices are affected and protecting them, not trying to sniff their traffic since it's been a long day. My proxy thus just intercepted and forwarded the traffic upstream using a fresh TLS connection that does not use the compromised keys.
=====
The flaw is egregious enough that the root cert which signed them (emSign ECC Root CA – G3) is getting removed from trust stores. We know 8 ridiculously weak keys got made, but we don't know that 0 slightly less weak but still unacceptable keys got made. What if they issued a CA key with 80 bits of entropy? That's enough that brute force would take more time than it's been since the disclosure, but not so long that they'd be considered at all secure (they should have 384 bits of entropy). So everything that root signed is untrustworthy, possibly everything the CA has signed. The G3 root is from 2018 IIRC, so lots of potentially exposed devices.
=====
This looks like it's using the emSign root path, which is useful, but the AAA root path is equally useful in different cases. The AAA root path was disabled for https on root stores newer than February 2025, but has been in root stores since forever. Meanwhile the emSign root path was only included in root stores since \~2018 and got removed in October 2026.

So from the perspective of hacking old IoT devices, if your device hasn't been updated since 2018, the AAA path should work; if since February 2025, either path works; if since October 2026, the emSign path.
=====
I didn't expect this commit to end up on HN, otherwise I would have done the full integration in one go. :) [https://github.com/mitmproxy/mitmproxy/pull/8503](https://github.com/mitmproxy/mitmproxy/pull/8503) adds the AAA root path.
=====
[https://en.wikipedia.org/wiki/EMudhra#2026\_intermediate\_cert...](https://en.wikipedia.org/wiki/EMudhra#2026_intermediate_certificate_compromise)

\> On October 17, 2026, browser root programs received a report that the private keys of eight eMudhra intermediate certificates had been compromised. The private keys were 4, 56, 66, 115, 155, 156, 230, and 237. A P-384 elliptic-curve private key normally has about 384 bits of entropy, but these keys had between 3 and 8 bits.
=====
FWIW i *havent* updated my ios, I'm on 26.6.2, and they all got revoked for me. The revocation shouldn't require an iOS update.
=====
It's a weird move for this post to simultaneously line this vulnerability up against Log4Shell (very few serversides are going to measure up to L4S) *and* insinuate that it's not practically exploitable at all.

If it was just "everyone take a breath, we've been through something worse with Log4Shell", that would make a lot more sense.
=====
This is so fucked up. I was reviewing my account and I just saw this in the logs... Claude says database credentials and bucket credentials go in via env vars so likely they had access to database and bucket contents.

```
  Audit Logs
  Oct 21, 2026 05:54:13 PM CDT
  BucketInstanceCredential.regenerated in my-project production by Railway Staff
  Oct 21, 2026 05:54:13 PM CDT
  BucketInstanceCredential.regenerated in my-project production by Railway Staff
  Oct 21, 2026 05:54:13 PM CDT
  BucketInstanceCredential.regenerated in my-project production by Railway Staff
```
=====
```
  > We even stood up a replication feed of their own control-plane database using credentials committed in their own repositories — GCP service-account keys and an Azure service principal — and let their infrastructure deliver it to us. Every six hours, for the last twenty-two days of the campaign, from 8 September to 30 September, that pipeline faithfully re-exported every table of the operational heart of TiDB Cloud — tenants, users, API keys, cluster credentials, changefeeds, migrations — and pushed it out. Eighty-eight consecutive exports, not one detection event.
```
I can’t even