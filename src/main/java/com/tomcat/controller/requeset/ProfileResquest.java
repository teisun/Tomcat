package com.tomcat.controller.requeset;

import lombok.AllArgsConstructor;
import lombok.Data;

@Data
@AllArgsConstructor
public class ProfileResquest {

    private Long userId;

    private String motherTongue;

    private String languageDepth;

    private String communicationStyle;

    private String targetLanguage;

}
