export interface BilibiliAccount { uid: number; name: string; vip: boolean; vipDue: number; verifiedAt: number }
export interface AccountStatus { state: "signedOut" | "saved" | "signedIn" | "expired"; account: BilibiliAccount | null }
export interface LoginQr { ticket: string; pixels: boolean[][]; expiresAt: number }
export interface LoginPoll { state: "waiting" | "scanned" | "signedIn" | "expired"; account: BilibiliAccount | null }
