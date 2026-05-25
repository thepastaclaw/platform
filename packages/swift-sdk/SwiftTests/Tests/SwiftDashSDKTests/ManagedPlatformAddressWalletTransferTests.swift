import XCTest
@testable import SwiftDashSDK

final class ManagedPlatformAddressWalletTransferTests: XCTestCase {

    func testCanonicalTransferOutputPlanTargetsChangeAfterSorting() {
        let recipient = ManagedPlatformAddressWallet.TransferOutput(
            addressType: 0,
            hash: Data(repeating: 0xAA, count: 20),
            credits: 50
        )
        let secondRecipient = ManagedPlatformAddressWallet.TransferOutput(
            addressType: 0,
            hash: Data(repeating: 0xFF, count: 20),
            credits: 75
        )
        let change = ManagedPlatformAddressWallet.ChangeAddress(
            addressType: 0,
            hash: Data(repeating: 0x01, count: 20)
        )

        let plan = ManagedPlatformAddressWallet.canonicalTransferOutputPlan(
            outputs: [recipient, secondRecipient],
            changeAddress: change,
            changeAmount: 125
        )

        XCTAssertEqual(plan.changeIndex, 0, "Change should move ahead of recipients after canonical sorting")
        XCTAssertNotEqual(plan.changeIndex, 2, "Fee targeting must not use the old last-insertion index")
        XCTAssertEqual(plan.rows.map(\.balance), [125, 50, 75])
        XCTAssertEqual(plan.rows.map { $0.address.address_type }, [0, 0, 0])
        XCTAssertEqual(plan.rows.map { withUnsafeBytes(of: $0.address.hash) { Data($0) } }, [
            Data(repeating: 0x01, count: 20),
            Data(repeating: 0xAA, count: 20),
            Data(repeating: 0xFF, count: 20)
        ])
    }
}
