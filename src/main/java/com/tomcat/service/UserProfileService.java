package com.tomcat.service;

import com.tomcat.controller.requeset.ProfileResquest;
import com.tomcat.controller.response.ProfileResponse;

public interface UserProfileService {

  ProfileResponse getByUserId(Long userId);

  ProfileResponse update(ProfileResquest resquest);

}