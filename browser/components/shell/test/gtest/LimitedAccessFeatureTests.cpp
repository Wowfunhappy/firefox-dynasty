/* -*- Mode: C++; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 2 -*- */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include "gtest/gtest.h"

#include "nsILimitedAccessFeature.h"
#include "nsServiceManagerUtils.h"
#include "WinUtils.h"

TEST(LimitedAccessFeature, VerifyGeneratedInfo)
{
  // If running on MSIX we have no guarantee that the
  // generated LAF info will match the known values.
  if (mozilla::widget::WinUtils::HasPackageIdentity()) {
    return;
  }

  const auto featureId = "com.microsoft.windows.taskbar.pin"_ns;
  const auto* token = "kRFiWpEK5uS6PMJZKmR7MQ==";
  const auto* attestation =
      "pcsmm0jrprpb2 has registered their use of "
      "com.microsoft.windows.taskbar.pin with Microsoft and agrees to the "
      "terms of use.";

  nsCOMPtr<nsILimitedAccessFeatureService> lafService =
      do_GetService("@mozilla.org/limited-access-feature-service;1");
  nsCOMPtr<nsILimitedAccessFeature> feature;
  nsresult rv = lafService->GenerateLimitedAccessFeature(
      featureId, getter_AddRefs(feature));
  ASSERT_TRUE(NS_SUCCEEDED(rv));

  nsAutoCString str;

  rv = feature->GetFeatureId(str);
  ASSERT_TRUE(NS_SUCCEEDED(rv));
  EXPECT_STREQ(featureId.get(), str.get());

  rv = feature->GetToken(str);
  ASSERT_TRUE(NS_SUCCEEDED(rv));
  EXPECT_STREQ(token, str.get());

  rv = feature->GetAttestation(str);
  ASSERT_TRUE(NS_SUCCEEDED(rv));
  EXPECT_STREQ(attestation, str.get());
}
