package com.tomcat.service;

import com.tomcat.domain.UserProfile;

public interface UserProfileService {

  UserProfile getByUserId(Long userId);

  UserProfile update(UserProfile profile);

}