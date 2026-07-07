import Foundation
import SwiftDashSDK

/// Wraps a `ManagedPlatformWallet` and a `ManagedCoreWallet`
final class TestWalletWrapper {
    private let core: ManagedCoreWallet
    private let wallet: ManagedPlatformWallet

    init(wallet: ManagedPlatformWallet, core: ManagedCoreWallet) {
        self.wallet = wallet
        self.core = core
    }

    func getCoreWallet() -> ManagedCoreWallet {
        core
    }

    func getPlatformWallet() -> ManagedPlatformWallet {
        wallet
    }

    /// Build, sign, and broadcast a BIP44 payment from this wallet,
    /// returning the serialized signed transaction — the flow the
    /// removed `ManagedCoreWallet.sendToAddresses` performed before
    /// #3970 moved core sends onto `CoreTransactionBuilder`. Kept as a
    /// test-support shorthand so the integration tests read as
    /// "send X to Y" rather than five builder steps.
    func sendToAddresses(
        recipients: [(address: String, amountDuffs: UInt64)],
        accountIndex: UInt32 = 0
    ) throws -> Data {
        let core = getCoreWallet()
        let platform = getPlatformWallet()
        let builder = try CoreTransactionBuilder(network: core.network())
        for recipient in recipients {
            try builder.addOutput(
                address: recipient.address,
                amountDuffs: recipient.amountDuffs
            )
        }
        try builder.setFunding(
            wallet: platform, accountType: .bip44, accountIndex: accountIndex
        )
        let signed = try builder.buildSigned(
            wallet: platform, accountType: .bip44, accountIndex: accountIndex
        )
        _ = try core.broadcastTransaction(signed)
        return signed.data
    }

    func waitForSpendable(exactly duffs: UInt64, timeout: TimeInterval = 60) async throws {
        try await Wait.until(
            "wallet spendable == \(duffs) duffs",
            timeout: timeout,
            pollInterval: 0.01
        ) {
            try wallet.balance().spendable == duffs
        }
    }
}

enum Wait {
    struct TimeoutError: Error, CustomStringConvertible {
        let description: String
    }

    static func until(
    _ message: @autoclosure () -> String,
    timeout: TimeInterval = 60,
    pollInterval: TimeInterval = 0.5,
    _ condition: () async throws -> Bool
    ) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if try await condition() { return }
            try await Task.sleep(nanoseconds: UInt64(pollInterval * 1_000_000_000))
        }
        throw TimeoutError(description: "Timed out after \(timeout)s waiting for: \(message())")
    }
}

@MainActor
extension PlatformWalletManager {
    func waitUntilUpToDate(height: Int, timeout: TimeInterval = 10) async throws {
        let target = UInt32(max(0, height))
        let deadline = Date().addingTimeInterval(timeout)
        var lastHeaders: UInt32 = 0
        var lastFilters: UInt32 = 0

        while Date() < deadline {
            let progress = try syncProgress()
            lastHeaders = progress.headers?.currentHeight ?? 0
            lastFilters = progress.filters?.currentHeight ?? 0

            if lastHeaders >= target && lastFilters >= target { return }

            try await Task.sleep(nanoseconds: 100_000_000)
        }

        throw SPVTestWaitError.timeout(
            "SPV did not reach height \(target) within \(timeout)s — " +
            "headers=\(lastHeaders), filters=\(lastFilters)"
        )
    }
}

enum SPVTestWaitError: LocalizedError {
    case timeout(String)

    var errorDescription: String? {
        switch self {
        case .timeout(let msg): return msg
        }
    }
}