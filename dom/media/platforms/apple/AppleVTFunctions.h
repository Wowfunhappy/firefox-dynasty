/* -*- Mode: C++; tab-width: 2; indent-tabs-mode: nil; c-basic-offset: 2 -*- */
/* vim:set ts=2 sw=2 sts=2 et cindent: */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// Construct references to each of the VideoToolbox symbols we use.
LINK_FUNC(VTDecompressionSessionCreate)
LINK_FUNC(VTDecompressionSessionDecodeFrame)
LINK_FUNC(VTDecompressionSessionInvalidate)
LINK_FUNC(VTDecompressionSessionWaitForAsynchronousFrames)
LINK_FUNC(VTSessionCopyProperty)
LINK_FUNC(VTSessionCopySupportedPropertyDictionary)
LINK_FUNC(VTSessionSetProperty)
LINK_FUNC(VTCompressionSessionEncodeFrame)
LINK_FUNC(VTCompressionSessionCreate)
LINK_FUNC(VTCompressionSessionInvalidate)
LINK_FUNC(VTCompressionSessionCompleteFrames)
